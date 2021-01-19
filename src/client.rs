use {
    super::error::{Error, Result},
    futures::prelude::*,
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
        Err(Error::CannotCreateClient {
            desc: "User Agent mustn't be empty".into(),
        })
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_bytes(user_agent.as_ref())?,
        );

        Ok(headers)
    }
}

/// Client struct.
#[derive(Debug)]
pub struct Client {
    mutex: Arc<Mutex<()>>,
    url: String,
    pub(crate) client: reqwest::Client,
    headers: HeaderMap,
}

impl Client {
    /// Create a new client with the specified value for the User-Agent header. The API requires a
    /// non-empty User-Agent header for all requests, preferably including your E621 username and
    /// the name of your project.
    pub fn new(url: &str, user_agent: impl AsRef<[u8]>) -> Result<Self> {
        Ok(Client {
            mutex: Arc::new(Mutex::new(())),
            url: url.to_string(),
            client: reqwest::Client::new(),
            headers: create_header_map(user_agent)?,
        })
    }

    /// Create a new client with the specified User-Agent header and proxy. The API requires a
    /// non-empty User-Agent header for all requests, preferably including your E621 username and
    /// the name of your project.
    pub fn with_proxy(url: &str, user_agent: impl AsRef<[u8]>, proxy: &str) -> Result<Self> {
        Ok(Client {
            client: reqwest::Client::builder().proxy(Proxy::https(proxy).map_err(|_| Error::CannotCreateClient { desc: "Invalid proxy address".into() })?).build().map_err(|_|
                Error::CannotCreateClient {
                    desc: "TLS backend cannot be initialized, or the resolver cannot load the system configuration".into()
                }
            )?,
            ..Client::new(url, user_agent)?
        })
    }

    pub fn get_json_endpoint(
        &self,
        endpoint: &str,
    ) -> impl Future<Output = Result<serde_json::Value>> {
        let url = format!("{}{}", self.url, endpoint);
        let request = self.client.get(&url).headers(self.headers.clone()).send();

        let c_mutex = self.mutex.clone();

        async move {
            {
                // we must lock the mutex when sleeping, just in case other stuff is going on in other threads
                let _lock = c_mutex.lock().unwrap();

                // wait first to make sure we're not exceeding the limit
                std::thread::sleep(REQ_COOLDOWN_DURATION);
            }

            match request.await {
                Ok(res) => {
                    if res.status().is_success() {
                        match res.json().await {
                            Ok(v) => Ok(v),
                            Err(e) => Err(Error::Serial {
                                desc: format!("{}", e),
                            }),
                        }
                    } else {
                        Err(Error::Http {
                            url,
                            code: res.status().as_u16(),
                            reason: match res.json::<serde_json::Value>().await {
                                Ok(v) => v["reason"].as_str().map(ToString::to_string),
                                Err(_) => None,
                            },
                        })
                    }
                }

                Err(e) => Err(Error::CannotSendRequest {
                    desc: format!("{}", e),
                }),
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

        assert_eq!(
            client.get_json_endpoint("/post/show.json?id=8595").await,
            Err(crate::error::Error::Http {
                url: format!("{}/post/show.json?id=8595", mockito::server_url()),
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
        assert!(
            Client::with_proxy(&mockito::server_url(), b"rs621/unit/test", "invalid_proxy")
                .is_err()
        );
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
