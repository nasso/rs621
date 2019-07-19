use super::error::{Error, Result};
use super::post::Post;

use std::convert::TryFrom;

use reqwest_mock::header::{self, HeaderMap, HeaderValue};

/// Maximum value allowed by the API for the `limit` option.
pub const LIST_HARD_LIMIT: u64 = 320;

/// Chunk size used for iterators performing requests
const ITER_CHUNK_SIZE: u64 = LIST_HARD_LIMIT;

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
            header::USER_AGENT,
            HeaderValue::from_bytes(user_agent.as_ref())?,
        );

        Ok(headers)
    }
}

/// A search query. Contains information about the tags used and an URL encoded version of the tags.
#[derive(Debug, PartialEq, Clone)]
pub struct Query {
    str_url: String,
    ordered: bool,
}

impl From<&[&str]> for Query {
    fn from(q: &[&str]) -> Query {
        let query_str = q.join(" ");
        let str_url = urlencoding::encode(&query_str);
        let ordered = q.iter().any(|t| t.starts_with("order:"));

        Query { str_url, ordered }
    }
}

/// Iterator returning posts from a search query.
#[derive(Debug)]
pub struct PostIter<'a, C> {
    client: &'a Client<C>,
    query: Query,

    last_id: Option<u64>,
    page: u64,
    chunk: Vec<Result<Post>>,
    ended: bool,
}

impl<'a, C> PostIter<'a, C> {
    fn new<T: Into<Query>>(
        client: &'a Client<C>,
        query: T,
        start_id: Option<u64>,
    ) -> PostIter<'a, C> {
        PostIter {
            client: client,

            query: query.into(),

            last_id: start_id,
            page: 0,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl<'a, C: reqwest_mock::Client> Iterator for PostIter<'a, C> {
    type Item = Result<Post>;

    fn next(&mut self) -> Option<Result<Post>> {
        // check if we need to load a new chunk of results
        if self.chunk.is_empty() {
            // get the JSON
            match self.client.get_json(&format!(
                "https://e621.net/post/index.json?limit={}{}&tags={}",
                ITER_CHUNK_SIZE,
                if self.query.ordered {
                    self.page += 1;
                    format!("&page={}", self.page)
                } else {
                    match self.last_id {
                        Some(i) => format!("&before_id={}", i),
                        None => String::new(),
                    }
                },
                self.query.str_url
            )) {
                Ok(body) => {
                    // put everything in the chunk
                    self.chunk = body
                        .as_array()
                        .unwrap()
                        .iter()
                        .rev()
                        .map(Post::try_from)
                        .collect()
                }

                // if something goes wrong, make the chunk be a single Err, and end the iterator
                Err(e) => {
                    self.ended = true;
                    self.chunk = vec![Err(e)]
                }
            }
        }

        // it's over if the chunk is still empty
        self.ended |= self.chunk.is_empty();

        if !self.ended {
            // get a post
            let post = self.chunk.pop().unwrap();

            // if there's an actual post, then it's the new last_id
            if let Ok(ref p) = post {
                self.last_id = Some(p.id);
            }

            // give them the post because we're nice
            Some(post)
        } else {
            // pops any eventual error
            // Vec::pop returns None if the Vec is empty anyway
            self.chunk.pop()
        }
    }
}

/// Client struct.
#[derive(Debug)]
pub struct Client<C> {
    client: C,
    headers: HeaderMap,
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
    /// This function performs a request; it will be subject to a short sleep time to ensure that
    /// the API rate limit isn't exceeded.
    pub fn get_post(&self, id: u64) -> Result<Post> {
        let body = self.get_json(&format!("https://e621.net/post/show.json?id={}", id))?;

        Post::try_from(&body)
    }

    /// Performs a search with the given tags and returns an iterator over the results.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::{Post, PostRating};
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let mut posts = client.list(&["fluffy", "rating:s"][..]);
    ///
    /// assert_eq!(posts.next().unwrap().unwrap().rating, PostRating::Safe);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    ///
    /// [`LIST_HARD_LIMIT`]: constant.LIST_HARD_LIMIT.html
    pub fn list<'a, T: Into<Query>>(&'a self, tags: T) -> PostIter<'a, C> {
        PostIter::new(self, tags, None)
    }

    /// List at most `limit` posts with IDs lower than `before_id` matching the tags. `limit` must
    /// be below [`LIST_HARD_LIMIT`].
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let posts: Vec<_> = client
    ///     .list_before(&["fluffy", "rating:s"][..], 123456)
    ///     .take(5)
    ///     .collect();
    ///
    /// assert_eq!(posts.len(), 5);
    /// assert!(posts[0].as_ref().unwrap().id < 123456);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    ///
    /// [`LIST_HARD_LIMIT`]: constant.LIST_HARD_LIMIT.html
    pub fn list_before<'a, T: Into<Query>>(&'a self, tags: T, before_id: u64) -> PostIter<'a, C> {
        PostIter::new(self, tags, Some(before_id))
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
    fn list_ordered() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        const REQ_TAGS: &str = "fluffy%20rating%3As%20order%3Ascore";

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&page=1&tags={}",
                    ITER_CHUNK_SIZE, REQ_TAGS
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(include_str!(
                "mocked/320_page-1_fluffy_rating-s_order-score.json"
            ))
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list(&["fluffy", "rating:s", "order:score"][..])
                .take(100)
                .collect::<Vec<_>>(),
            serde_json::from_str::<JsonValue>(include_str!(
                "mocked/320_page-1_fluffy_rating-s_order-score.json"
            ))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .take(100)
            .map(Post::try_from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn list_above_limit_ordered() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        const REQ_TAGS: &str = "fluffy%20rating%3As%20order%3Ascore";

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&page=1&tags={}",
                    ITER_CHUNK_SIZE, REQ_TAGS
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(include_str!(
                "mocked/320_page-1_fluffy_rating-s_order-score.json"
            ))
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&page=2&tags={}",
                    ITER_CHUNK_SIZE, REQ_TAGS
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(include_str!(
                "mocked/320_page-2_fluffy_rating-s_order-score.json"
            ))
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list(&["fluffy", "rating:s", "order:score"][..])
                .take(400)
                .collect::<Vec<_>>(),
            serde_json::from_str::<JsonValue>(include_str!(
                "mocked/320_page-1_fluffy_rating-s_order-score.json"
            ))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .chain(
                serde_json::from_str::<JsonValue>(include_str!(
                    "mocked/320_page-2_fluffy_rating-s_order-score.json"
                ))
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
            )
            .take(400)
            .map(Post::try_from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn list_before_id() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/320_before-1869409_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .take(80)
            .map(Post::try_from)
            .collect();

        let request = Url::parse(&format!(
            "https://e621.net/post/index.json?limit={}&before_id=1869409&tags=fluffy%20rating%3As",
            ITER_CHUNK_SIZE
        ))
        .unwrap();

        assert!(client
            .client
            .stub(request)
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list_before(&["fluffy", "rating:s"][..], 1869409)
                .take(80)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn list_above_limit() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/400_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(Post::try_from)
            .collect();

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&tags=fluffy%20rating%3As",
                    ITER_CHUNK_SIZE
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/320_fluffy_rating-s.json"))
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&before_id={}&tags={}",
                    ITER_CHUNK_SIZE, 1869409, "fluffy%20rating%3As"
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(include_str!(
                "mocked/320_before-1869409_fluffy_rating-s.json"
            ))
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list(&["fluffy", "rating:s"][..])
                .take(400)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn list_no_result() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = "[]";
        let expected = Vec::new();

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&tags=fluffy%20rating%3As",
                    ITER_CHUNK_SIZE
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list(&["fluffy", "rating:s"][..])
                .take(5)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn list_simple() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/320_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .take(5)
            .map(Post::try_from)
            .collect();

        assert!(client
            .client
            .stub(
                Url::parse(&format!(
                    "https://e621.net/post/index.json?limit={}&tags=fluffy%20rating%3As",
                    ITER_CHUNK_SIZE
                ))
                .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(
            client
                .list(&["fluffy", "rating:s"][..])
                .take(5)
                .collect::<Vec<_>>(),
            expected
        );
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

        let response = include_str!("mocked/id_8595.json");
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
