use super::error::{Error, Result};
use futures::prelude::*;
use reqwest::header::{HeaderMap, HeaderValue};

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
            url: url.to_string(),
            client: reqwest::Client::new(),
            headers: create_header_map(user_agent)?,
        })
    }

    pub fn get_json_endpoint(
        &self,
        endpoint: &str,
    ) -> impl Future<Output = Result<serde_json::Value>> {
        let request = self
            .client
            .get(&format!("{}{}", self.url, endpoint))
            .headers(self.headers.clone())
            .send();

        async move {
            // Wait first to make sure we're not exceeding the limit
            std::thread::sleep(REQ_COOLDOWN_DURATION);

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
    use futures::executor::block_on;
    use mockito::mock;

    #[tokio::test]
    async fn get_json_endpoint_http_error() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let _m = mock("GET", "/post/show.json?id=8595")
            .with_status(500)
            .with_body(r#"{"success":false,"reason":"foo"}"#)
            .create();

        assert_eq!(
            block_on(client.get_json_endpoint("/post/show.json?id=8595")),
            Err(crate::error::Error::Http {
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
            block_on(client.get_json_endpoint("/post/show.json?id=8595")),
            Ok({
                let mut m = serde_json::Map::new();
                m.insert(String::from("dummy"), "json".into());
                m.into()
            })
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
