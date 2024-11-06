#[cfg(all(target_family = "wasm", feature = "rate-limit"))]
#[path = "client/gloo_rate_limit.rs"]
mod rate_limit;

#[cfg(all(not(target_family = "wasm"), feature = "rate-limit"))]
#[path = "client/tokio_rate_limit.rs"]
mod rate_limit;

#[cfg(not(feature = "rate-limit"))]
#[path = "client/dummy_rate_limit.rs"]
mod rate_limit;

/// Forced cool down duration performed at every request. E621 allows at most 2 requests per second,
/// so the lowest safe value we can have here is 500 ms.
#[cfg(feature = "rate-limit")]
const REQ_COOLDOWN_DURATION: std::time::Duration = std::time::Duration::from_millis(600);

use crate::error::{Error, Result};

use futures::Future;

use reqwest::header::HeaderMap;
use reqwest::{Response, Url};

use serde::Serialize;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use std::{fmt, num::ParseIntError, str::FromStr};

#[cfg(target_family = "wasm")]
fn create_header_map<T: AsRef<[u8]>>(_user_agent: T) -> Result<HeaderMap> {
    Ok(HeaderMap::new())
}

#[cfg(not(target_family = "wasm"))]
fn create_header_map<T: AsRef<[u8]>>(user_agent: T) -> Result<HeaderMap> {
    if user_agent.as_ref() == b"" {
        Err(Error::CannotCreateClient(String::from(
            "User Agent mustn't be empty",
        )))
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_bytes(user_agent.as_ref())
                .map_err(|e| Error::InvalidHeaderValue(format!("{}", e)))?,
        );

        Ok(headers)
    }
}

#[cfg(not(target_family = "wasm"))]
fn create_extra_query<T: AsRef<[u8]>>(_user_agent: T) -> Result<Vec<(String, String)>> {
    Ok(Default::default())
}

#[cfg(target_family = "wasm")]
fn create_extra_query<T: AsRef<[u8]>>(user_agent: T) -> Result<Vec<(String, String)>> {
    let value = std::str::from_utf8(user_agent.as_ref())
        .map_err(|e| Error::InvalidHeaderValue(format!("{}", e)))?;

    Ok(vec![("_client".into(), value.into())])
}

#[cfg(not(target_family = "wasm"))]
pub(crate) type QueryFuture = Box<dyn Future<Output = Result<serde_json::Value>> + Send>;

#[cfg(target_family = "wasm")]
pub(crate) type QueryFuture = Box<dyn Future<Output = Result<serde_json::Value>>>;

/// Where to begin returning results from in paginated requests.
#[derive(Debug, PartialEq, Eq, Clone, Copy, SerializeDisplay, DeserializeFromStr)]
pub enum Cursor {
    /// Begin at the given page. Actual offset depends on page size.
    Page(u64),

    /// Return page size items ordered before the given id.
    Before(u64),

    /// Return page size items ordered after the given id.
    After(u64),
}

impl FromStr for Cursor {
    type Err = ParseIntError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let result = match s.chars().next() {
            Some('a') => Self::After(s[1..].parse()?),
            Some('b') => Self::Before(s[1..].parse()?),
            None | Some(_) => Self::Page(s.parse()?),
        };

        Ok(result)
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(p) => write!(f, "{p}"),
            Self::Before(p) => write!(f, "b{p}"),
            Self::After(p) => write!(f, "a{p}"),
        }
    }
}

/// Client struct.
#[derive(Debug)]
pub struct Client {
    pub(crate) client: reqwest::Client,
    rate_limit: rate_limit::RateLimit,
    url: Url,
    headers: HeaderMap,
    extra_query: Vec<(String, String)>,
    login: Option<(String, String)>,
}

impl Client {
    fn create(url: &str, user_agent: impl AsRef<[u8]>, proxy: Option<&str>) -> Result<Self> {
        let client = reqwest::Client::builder();
        let client = match proxy {
            #[cfg(target_family = "wasm")]
            Some(_) => panic!("proxies are not supported in wasm"),

            #[cfg(not(target_family = "wasm"))]
            Some(proxy) => {
                let proxy = reqwest::Proxy::https(proxy)
                    .map_err(|e| Error::CannotCreateClient(format!("{}", e)))?;

                client.proxy(proxy)
            }

            None => client,
        };

        let client = client
            .build()
            .map_err(|e| Error::CannotCreateClient(format!("{}", e)))?;

        Ok(Client {
            client,
            url: Url::parse(url)?,
            rate_limit: Default::default(),
            headers: create_header_map(&user_agent)?,
            extra_query: create_extra_query(&user_agent)?,
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

    pub(crate) fn url(&self, endpoint: &str) -> Result<Url, url::ParseError> {
        let mut url = self.url.join(endpoint)?;
        if let Some((ref login, ref api_key)) = self.login {
            url.query_pairs_mut()
                .append_pair("login", login)
                .append_pair("api_key", api_key);
        }

        for (key, value) in &self.extra_query {
            url.query_pairs_mut().append_pair(key, value);
        }

        Ok(url)
    }

    async fn post_response<T>(&self, endpoint: &str, body: &T) -> Result<Response>
    where
        T: serde::Serialize,
    {
        let url = self.url(endpoint)?;
        let mut request = self.client.post(url.clone());

        if let Some((ref username, ref password)) = self.login {
            request = request.basic_auth(username, Some(password));
        }

        let request_fut = request
            .form(body) // `.json(...)` has problems with CORS in WASM.
            .headers(self.headers.clone())
            .send();

        self.rate_limit
            .clone()
            .check(async move {
                let res = request_fut
                    .await
                    .map_err(|e| Error::CannotSendRequest(format!("{}", e)))?;

                if res.status().is_success() {
                    Ok(res)
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
            })
            .await
    }

    pub(crate) async fn post_form<T>(&self, endpoint: &str, body: &T) -> Result<serde_json::Value>
    where
        T: serde::Serialize,
    {
        self.post_response(endpoint, body)
            .await?
            .json()
            .await
            .map_err(|e| Error::Serial(format!("{e}")))
    }

    pub(crate) async fn delete(&self, endpoint: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Form {
            _method: &'static str,
        }

        // Can't use HTTP DELETE because e621's CORS headers aren't permissive enough. Thankfully
        // ruby on rails has a workaround for exactly this purpose.
        self.post_response(endpoint, &Form { _method: "delete" })
            .await?;
        Ok(())
    }

    pub(crate) async fn get_json_endpoint_query<T, R>(&self, endpoint: &str, query: &T) -> Result<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let url = self.url(endpoint)?;
        let future = self
            .client
            .get(url.clone())
            .query(query)
            .headers(self.headers.clone())
            .send();

        let res = self
            .rate_limit
            .clone()
            .check(future)
            .await
            .map_err(|x| Error::CannotSendRequest(x.to_string()))?;

        if res.status().is_success() {
            res.json()
                .await
                .map_err(|e| Error::Serial(format!("{}", e)))
        } else {
            Err(Error::Http {
                url: res.url().clone(),
                code: res.status().as_u16(),
                reason: match res.json::<serde_json::Value>().await {
                    Ok(v) => v["reason"].as_str().map(ToString::to_string),
                    Err(_) => None,
                },
            })
        }
    }

    pub fn get_json_endpoint(
        &self,
        endpoint: &str,
    ) -> impl Future<Output = Result<serde_json::Value>> {
        let url = self.url(endpoint);
        let request = url
            .clone()
            .map(|url| self.client.get(url).headers(self.headers.clone()).send());

        self.rate_limit.clone().check(async move {
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
        })
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
    async fn get_json_endpoint_query_success() {
        #[derive(Serialize)]
        struct Query {
            id: u64,
        }

        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let _m = mock("GET", "/post/show.json?id=8595")
            .with_body(r#"{"dummy":"json"}"#)
            .create();

        let query = Query { id: 8595 };
        assert_eq!(
            client
                .get_json_endpoint_query("/post/show.json", &query)
                .await,
            Ok(serde_json::json!({
                "dummy": "json",
            }))
        );
    }

    #[tokio::test]
    async fn get_json_endpoint_query_http_error() {
        #[derive(Serialize)]
        struct Query {
            id: u64,
        }

        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        // note: these are still using old endpoint but it doesn't matter here
        let _m = mock("GET", "/post/show.json?id=8595")
            .with_status(500)
            .with_body(r#"{"success":false,"reason":"foo"}"#)
            .create();

        let server_url = Url::parse(&mockito::server_url()).unwrap();

        let query = Query { id: 8595 };
        assert_eq!(
            client
                .get_json_endpoint_query::<_, serde_json::Value>("/post/show.json", &query)
                .await,
            Err(crate::error::Error::Http {
                url: server_url.join("/post/show.json?id=8595").unwrap(),
                code: 500,
                reason: Some(String::from("foo"))
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
