use super::error::{Error, Result};
use reqwest_mock::header::{self, HeaderMap, HeaderValue};

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
            header::USER_AGENT,
            HeaderValue::from_bytes(user_agent.as_ref())?,
        );

        Ok(headers)
    }
}

/// Client struct.
#[derive(Debug)]
pub struct Client<C: reqwest_mock::Client> {
    pub(crate) client: C,
    headers: HeaderMap,
}

impl<C: reqwest_mock::Client> Client<C> {
    pub(crate) fn get_json<U: reqwest_mock::IntoUrl>(&self, url: U) -> Result<serde_json::Value> {
        // Wait first to make sure we're not exceeding the limit
        ::std::thread::sleep(REQ_COOLDOWN_DURATION);

        match self.client.get(url).headers(self.headers.clone()).send() {
            Ok(res) => {
                if res.status.is_success() {
                    match res.body_to_utf8() {
                        Ok(s) => match serde_json::from_str(&s) {
                            Ok(v) => Ok(v),
                            Err(e) => Err(Error::Serial {
                                desc: format!("{}", e),
                            }),
                        },
                        Err(e) => Err(Error::Serial {
                            desc: format!("{}", e),
                        }),
                    }
                } else {
                    Err(Error::Http {
                        code: res.status.as_u16(),
                        reason: match res.body_to_utf8() {
                            Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                                Ok(v) => v["reason"].as_str().map(ToString::to_string),
                                Err(_) => None,
                            },
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

impl Client<reqwest_mock::DirectClient> {
    /// Create a new client with the specified value for the User-Agent header. The API requires a
    /// non-empty User-Agent header for all requests, preferably including your E621 username and
    /// the name of your project.
    pub fn new(user_agent: impl AsRef<[u8]>) -> Result<Self> {
        Ok(Client {
            client: reqwest_mock::DirectClient::new(),
            headers: create_header_map(user_agent)?,
        })
    }
}

#[cfg(test)]
impl Client<reqwest_mock::StubClient> {
    pub(crate) fn new_mocked(user_agent: impl AsRef<[u8]>) -> Result<Self> {
        use reqwest_mock::{StubClient, StubDefault, StubSettings, StubStrictness};

        Ok(Client {
            headers: create_header_map(user_agent)?,
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest_mock::{Method, StatusCode, Url};

    #[test]
    fn get_json_http_error() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/post/show.json?id=8595").unwrap())
            .method(Method::GET)
            .response()
            .status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .body(r#"{"success":false,"reason":"foo"}"#)
            .mock()
            .is_ok());

        assert_eq!(
            client.get_json("https://e621.net/post/show.json?id=8595"),
            Err(crate::error::Error::Http {
                code: 500,
                reason: Some(String::from("foo"))
            })
        );
    }

    #[test]
    fn get_json_works() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/post/show.json?id=8595").unwrap())
            .method(Method::GET)
            .response()
            .body(r#"{"dummy":"json"}"#)
            .mock()
            .is_ok());

        assert_eq!(
            client.get_json("https://e621.net/post/show.json?id=8595"),
            Ok({
                let mut m = serde_json::Map::new();
                m.insert(String::from("dummy"), "json".into());
                m.into()
            })
        );
    }

    #[test]
    fn create_header_map_works() {
        assert!(create_header_map(b"rs621/unit_test").is_ok());
    }

    #[test]
    fn create_header_map_requires_valid_user_agent() {
        assert!(create_header_map(b"\n").is_err());
    }

    #[test]
    fn create_header_map_requires_non_empty_user_agent() {
        assert!(create_header_map(b"").is_err());
    }
}
