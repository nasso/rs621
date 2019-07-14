use super::error::{Error, Result};
use super::post::Post;

use std::convert::TryFrom;

use reqwest_mock::header::{self, HeaderMap, HeaderValue};

/// Maximum value allowed by the API for the `limit` option.
static LIST_HARD_LIMIT: usize = 320;

/// Forced cool down duration performed at every request. E621 allows at most 2 requests per second,
/// so the lowest safe value we can have here is 500 ms.
const REQ_COOLDOWN_DURATION: ::std::time::Duration = ::std::time::Duration::from_millis(600);

/// Client struct.
#[derive(Debug)]
pub struct Client<C> {
    client: C,
    headers: HeaderMap,
}

fn create_header_map<T: AsRef<[u8]>>(user_agent: T) -> Result<HeaderMap> {
    if user_agent.as_ref() == b"" {
        Err(Error::CannotCreateClient(String::from(
            "User Agent mustn't be empty",
        )))
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_bytes(user_agent.as_ref())?,
        );

        Ok(headers)
    }
}

impl<C: reqwest_mock::Client> Client<C> {
    fn get_json<U: reqwest_mock::IntoUrl>(&self, url: U) -> Result<serde_json::Value> {
        // Wait first to make sure we're not exceeding the limit
        ::std::thread::sleep(REQ_COOLDOWN_DURATION);

        match self.client.get(url).headers(self.headers.clone()).send() {
            Ok(res) => {
                if res.status.is_success() {
                    match res.body_to_utf8() {
                        Ok(s) => match serde_json::from_str(&s) {
                            Ok(v) => Ok(v),
                            Err(e) => Err(Error::Serial(format!("{}", e))),
                        },
                        Err(e) => Err(Error::Serial(format!("{}", e))),
                    }
                } else {
                    Err(Error::Http(res.status.as_u16()))
                }
            }

            Err(e) => Err(Error::CannotSendRequest(format!("{}", e))),
        }
    }

    /// Returns the post with the given ID.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let post = client.get_post(8595)?;
    ///
    /// assert_eq!(post.id, 8595);
    /// # Ok(()) }
    /// ```
    ///
    /// This function performs a request; it will perform a short sleep to ensure that the API rate
    /// limit isn't exceeded.
    pub fn get_post(&self, id: u64) -> Result<Post> {
        let body = self.get_json(&format!("https://e621.net/post/show.json?id={}", id))?;

        Post::try_from(&body)
    }

    /// Performs a search on E621 with the given tags and returns at most `limit` results.
    ///
    /// The E621 API has a hard limit of 320 results per request. `rs621` can go beyond that limit
    /// and automatically performs more requests until enough posts are gathered (except for ordered
    /// queries, i.e. containing an `order:*` tag). The requests are performed sequentially and the
    /// function will therefore take a longer time to return.
    ///
    /// This function can perform more than one request; it will perform a short sleep before every
    /// request to ensure that the API rate limit isn't exceeded.
    pub fn list(&self, q: &[&str], limit: usize) -> Result<Vec<Post>> {
        let query_str = q.join(" ");
        let query_str_url = urlencoding::encode(&query_str);
        let ordered = q.iter().any(|t| t.starts_with("order:"));

        let mut posts = Vec::new();

        if ordered {
            if limit > LIST_HARD_LIMIT {
                return Err(Error::AboveLimit(limit, LIST_HARD_LIMIT));
            }

            let body = self.get_json(&format!(
                "https://e621.net/post/index.json?limit={}&tags={}",
                limit, query_str_url
            ))?;

            for p in body.as_array().unwrap().iter() {
                match Post::try_from(p) {
                    Ok(post) => posts.push(post),
                    _ => (),
                }
            }
        } else {
            let mut lowest_id = None;

            while posts.len() < limit {
                let left = limit - posts.len();
                let batch = left.min(LIST_HARD_LIMIT);

                let body = self.get_json(&format!(
                    "https://e621.net/post/index.json?limit={}&tags={}{}",
                    batch,
                    query_str_url,
                    if let Some(i) = lowest_id {
                        format!("&before_id={}", i)
                    } else {
                        "".to_string()
                    }
                ))?;

                let post_array = body.as_array().unwrap();

                if post_array.is_empty() {
                    break;
                }

                for p in post_array.iter() {
                    match Post::try_from(p) {
                        Ok(post) => {
                            if let Some(i) = lowest_id {
                                lowest_id = Some(post.id.min(i));
                            } else {
                                lowest_id = Some(post.id);
                            }

                            posts.push(post);
                        }
                        _ => (),
                    }
                }
            }
        }

        Ok(posts)
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
mod tests {
    use super::*;
    use reqwest_mock::{
        Method, StatusCode, StubClient, StubDefault, StubSettings, StubStrictness, Url,
    };

    use serde_json::Value as JsonValue;

    #[test]
    fn list_post() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/index_fluffy_rating-s_limit-5.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<Post> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Post::try_from(v).unwrap())
            .collect();

        assert!(client
            .client
            .stub(
                Url::parse("https://e621.net/post/index.json?limit=5&tags=fluffy%20rating%3As")
                    .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(client.list(&["fluffy", "rating:s"], 5), Ok(expected));
    }

    #[test]
    fn get_post_by_id() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/show_id_8595.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected = Post::try_from(&response_json).unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/post/show.json?id=8595").unwrap())
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(client.get_post(8595), Ok(expected));
    }

    #[test]
    fn get_json_http_error() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/post/show.json?id=8595").unwrap())
            .method(Method::GET)
            .response()
            .status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .body("")
            .mock()
            .is_ok());

        assert_eq!(
            client.get_json("https://e621.net/post/show.json?id=8595"),
            Err(crate::error::Error::Http(500))
        );
    }

    #[test]
    fn get_json_works() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

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
