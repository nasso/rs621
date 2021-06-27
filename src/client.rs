use futures::Future;
use reqwest::Url;

use {
    super::error::{Error, Result},
    reqwest::{
        header::{HeaderMap, HeaderValue},
        Proxy,
    },
    std::sync::{Arc, Mutex},
};

/// Forced cool down duration performed at every request. E621 allows at most 2 requests per second,
/// so the lowest safe value we can have here is 500 ms.
const REQ_COOLDOWN_DURATION: ::std::time::Duration = ::std::time::Duration::from_millis(600);

fn create_header_map<T: AsRef<[u8]>>(user_agent: T) -> Result<HeaderMap> {
    if user_agent.as_ref() == b"" {
        Err(Error::CannotCreateClient(String::from(
            "User Agent mustn't be empty",
        )))
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_bytes(user_agent.as_ref())
                .map_err(|e| Error::InvalidHeaderValue(format!("{}", e)))?,
        );

        Ok(headers)
    }
}

/// Client struct.
#[derive(Debug)]
pub struct Client {
    pub(crate) client: reqwest::Client,
    mutex: Arc<Mutex<()>>,
    url: Url,
    headers: HeaderMap,
    login: Option<(String, String)>,
}

impl Client {
    fn create(url: &str, user_agent: impl AsRef<[u8]>, proxy: Option<&str>) -> Result<Self> {
        let mut client = reqwest::Client::builder();

        if let Some(proxy) = proxy {
            let proxy =
                Proxy::https(proxy).map_err(|e| Error::CannotCreateClient(format!("{}", e)))?;

            client = client.proxy(proxy);
        }

        let client = client
            .build()
            .map_err(|e| Error::CannotCreateClient(format!("{}", e)))?;

        Ok(Client {
            client,
            url: Url::parse(url)?,
            mutex: Default::default(),
            headers: create_header_map(user_agent)?,
            login: None,
        })
    }

    /// Create a new client with the specified value for the User-Agent header. The API requires a
    /// non-empty User-Agent header for all requests, preferably including your E621 username and
    /// the name of your project.
    pub fn new(url: &str, user_agent: impl AsRef<[u8]>) -> Result<Self> {
        Client::create(url, user_agent, None)
    }

    /// Create a new client with the specified User-Agent header and proxy. The API requires a
    /// non-empty User-Agent header for all requests, preferably including your E621 username and
    /// the name of your project.
    pub fn with_proxy(url: &str, user_agent: impl AsRef<[u8]>, proxy: &str) -> Result<Self> {
        Client::create(url, user_agent, Some(proxy))
    }

    /// Login to the server with the provided username and API key. All subsequent requests will be
    /// sent with the given credentials.
    pub fn login(&mut self, username: String, api_key: String) {
        self.login = Some((username, api_key));
    }

    /// Remove any login information previously set with [Client::login].
    pub fn logout(&mut self) {
        self.login = None;
    }

    pub fn get_json_endpoint(
        &self,
        endpoint: &str,
    ) -> impl Future<Output = Result<serde_json::Value>> {
        let url = match self.url.join(endpoint) {
            Ok(mut url) => {
                if let Some((ref login, ref api_key)) = self.login {
                    url.query_pairs_mut()
                        .append_pair("login", login)
                        .append_pair("api_key", api_key);
                }

                Ok(url)
            }
            e => e,
        };

        let request = url
            .clone()
            .map(|url| self.client.get(url).headers(self.headers.clone()).send());

        let c_mutex = self.mutex.clone();

        {
            // we must lock the mutex when sleeping, just in case other stuff is going on in other threads
            let _lock = c_mutex.lock().unwrap();

            // wait first to make sure we're not exceeding the limit
            std::thread::sleep(REQ_COOLDOWN_DURATION);
        }

        async move {
            let res = request?
                .await
                .map_err(|e| Error::CannotSendRequest(format!("{}", e)))?;

            if res.status().is_success() {
                res.json()
                    .await
                    .map_err(|e| Error::Serial(format!("{}", e)))
            } else {
                Err(Error::Http {
                    url: url?,
                    code: res.status().as_u16(),
                    reason: match res.json::<serde_json::Value>().await {
                        Ok(v) => v["reason"].as_str().map(ToString::to_string),
                        Err(_) => None,
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[tokio::test]
    async fn get_json_endpoint_http_error() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        // note: these are still using old endpoint but it doesn't matter here
        let _m = mock("GET", "/post/show.json?id=8595")
            .with_status(500)
            .with_body(r#"{"success":false,"reason":"foo"}"#)
            .create();

        let server_url = Url::parse(&mockito::server_url()).unwrap();

        assert_eq!(
            client.get_json_endpoint("/post/show.json?id=8595").await,
            Err(crate::error::Error::Http {
                url: server_url.join("/post/show.json?id=8595").unwrap(),
                code: 500,
                reason: Some(String::from("foo"))
            })
        );
    }

    #[tokio::test]
    async fn get_json_endpoint_success() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let _m = mock("GET", "/post/show.json?id=8595")
            .with_body(r#"{"dummy":"json"}"#)
            .create();

        assert_eq!(
            client.get_json_endpoint("/post/show.json?id=8595").await,
            Ok({
                let mut m = serde_json::Map::new();
                m.insert(String::from("dummy"), "json".into());
                m.into()
            })
        );
    }

    #[tokio::test]
    async fn create_client_with_proxy_works() {
        assert!(Client::with_proxy(
            &mockito::server_url(),
            b"rs621/unit_test",
            &mockito::server_url()
        )
        .is_ok());

        #[cfg(feature = "socks")]
        assert!(Client::with_proxy(
            &mockito::server_url(),
            b"rs621/unit_test",
            &("socks5://".to_owned() + format!("{}", &mockito::server_address()).as_str())
        )
        .is_ok());
    }

    #[tokio::test]
    async fn create_client_with_invalid_proxy_fails() {
        assert!(Client::with_proxy(&mockito::server_url(), b"rs621/unit/test", "").is_err());
    }

    #[tokio::test]
    async fn create_header_map_works() {
        assert!(create_header_map(b"rs621/unit_test").is_ok());
    }

    #[tokio::test]
    async fn create_header_map_requires_valid_user_agent() {
        assert!(create_header_map(b"\n").is_err());
    }

    #[tokio::test]
    async fn create_header_map_requires_non_empty_user_agent() {
        assert!(create_header_map(b"").is_err());
    }
}
