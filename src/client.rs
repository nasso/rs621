use super::error::{Error, Result};
use super::post::Post;

use std::convert::TryFrom;

use reqwest_mock::header::{self, HeaderMap, HeaderValue};

/// Maximum value allowed by the API for the `limit` option.
pub const LIST_HARD_LIMIT: u64 = 320;

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

fn check_limit(limit: u64) -> Result<()> {
    // check that the limit is <= 320
    if limit > LIST_HARD_LIMIT {
        Err(Error::AboveLimit(
            String::from("limit"),
            limit,
            LIST_HARD_LIMIT,
        ))
    } else {
        Ok(())
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

    /// List at most `limit` posts matching the tags. `limit` must be below [`LIST_HARD_LIMIT`].
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let posts = client.list(&["fluffy", "rating:s"][..], 5)?;
    ///
    /// assert_eq!(posts.len(), 5);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    ///
    /// [`LIST_HARD_LIMIT`]: constant.LIST_HARD_LIMIT.html
    pub fn list<T: Into<Query>>(&self, tags: T, limit: u64) -> Result<Vec<Post>> {
        check_limit(limit)?;

        // parse query
        let query = tags.into();

        // results
        let mut posts = Vec::new();

        // get the JSON
        let body = self.get_json(&format!(
            "https://e621.net/post/index.json?limit={}&tags={}",
            limit, query.str_url
        ))?;

        // put everything in the Vec<Post>
        for p in body.as_array().unwrap().iter() {
            posts.push(Post::try_from(p)?);
        }

        // yay
        Ok(posts)
    }

    /// List at most `limit` posts with IDs lower than `before_id` matching the tags. `limit` must
    /// be below [`LIST_HARD_LIMIT`].
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let posts = client.list_before(&["fluffy", "rating:s"][..], 5, 123456)?;
    ///
    /// assert_eq!(posts.len(), 5);
    /// assert!(posts[0].id < 123456);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    ///
    /// [`LIST_HARD_LIMIT`]: constant.LIST_HARD_LIMIT.html
    pub fn list_before<T: Into<Query>>(
        &self,
        tags: T,
        limit: u64,
        before_id: u64,
    ) -> Result<Vec<Post>> {
        check_limit(limit)?;

        // parse query
        let query = tags.into();

        // results
        let mut posts = Vec::new();

        // get the JSON
        let body = self.get_json(&format!(
            "https://e621.net/post/index.json?limit={}&before_id={}&tags={}",
            limit, before_id, query.str_url
        ))?;

        // put everything in the Vec<Post>
        for p in body.as_array().unwrap().iter() {
            posts.push(Post::try_from(p)?);
        }

        // yay
        Ok(posts)
    }

    /// Paginates the search query into pages of length `limit` and returns page `page`. `limit`
    /// must be below [`LIST_HARD_LIMIT`].
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let posts = client.list_page(&["fluffy", "rating:s"][..], 5, 4)?;
    ///
    /// assert_eq!(posts.len(), 5);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    ///
    /// [`LIST_HARD_LIMIT`]: constant.LIST_HARD_LIMIT.html
    pub fn list_page<T: Into<Query>>(&self, tags: T, limit: u64, page: u16) -> Result<Vec<Post>> {
        check_limit(limit)?;

        // parse query
        let query = tags.into();

        // results
        let mut posts = Vec::new();

        // get the JSON
        let body = self.get_json(&format!(
            "https://e621.net/post/index.json?limit={}&page={}&tags={}",
            limit, page, query.str_url
        ))?;

        // put everything in the Vec<Post>
        for p in body.as_array().unwrap().iter() {
            posts.push(Post::try_from(p)?);
        }

        // yay
        Ok(posts)
    }

    /// Performs a search on E621 with the given tags and returns at most `limit` results.
    ///
    /// The E621 API has a hard limit of 320 results per request. This function allows you to go
    /// beyond that limit by automatically making more requests until enough posts are gathered
    /// (except for ordered queries, i.e. containing an `order:*` tag). The requests are performed
    /// sequentially using [`list_before`] and the function might therefore take a longer time to
    /// return.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let posts = client.search(&["fluffy", "rating:s"][..], 400)?;
    ///
    /// assert_eq!(posts.len(), 400);
    /// # Ok(()) }
    /// ```
    ///
    /// Because this function uses [`list_before`] to go beyond the API hard limit, it isn't
    /// compatible with ordered queries. From the official API:
    ///
    /// > Sorting results with 'order:' using `tags` does nothing in conjunction with `before_id`.
    ///
    /// If one of the tags contains `order:`, this function is strictly equivalent to [`list`].
    ///
    /// This function can perform more than one request; it will perform a short sleep before every
    /// request to ensure that the API rate limit isn't exceeded.
    ///
    /// [`list`]: #method.list
    /// [`list_before`]: #method.list_before
    pub fn search<T: Into<Query>>(&self, q: T, limit: u64) -> Result<Vec<Post>> {
        let query = q.into();

        if query.ordered {
            // ordered searches are the same as just calling Client::list
            self.list(query, limit)
        } else {
            // the only reason anyone would call Client::search instead is to go beyond the limit
            let mut posts = Vec::new();

            let mut lowest_id = None;

            // repeat while we didn't reach `limit` posts
            while (posts.len() as u64) < limit {
                // how many posts are left
                let left = limit - posts.len() as u64;

                // how many we're going to get on this iteration of the loop
                let batch = left.min(LIST_HARD_LIMIT);

                // get the posts
                let page_posts = match lowest_id {
                    // on the first iteration we don't have to specify `before_id`
                    None => self.list(query.clone(), batch)?,

                    // we have to do so for every subsequent iteration though
                    Some(i) => self.list_before(query.clone(), batch, i)?,
                };

                // Yes I'm lazily cloning the query. If you manage to do something better please
                // make a PR.

                // don't forget to break as soon as we don't get any post!
                // that means there's no more result left
                if page_posts.is_empty() {
                    break;
                }

                // we could also exit early by just checking that we get exactly `batch` results
                // if we get any less, it means that we've reached the end of the results
                // i didn't do it here though, but maybe i will one day

                // for each post..
                for post in page_posts.into_iter() {
                    if let Some(i) = lowest_id {
                        // if it isn't the first one, update the value of the  smaller ID we've got
                        // so far
                        lowest_id = Some(post.id.min(i));
                    } else {
                        // if it's the first post, well it's just the smaller ID anyway
                        lowest_id = Some(post.id);
                    }

                    // add it to the result list
                    posts.push(post);
                }
            }

            Ok(posts)
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
mod tests {
    use super::*;
    use reqwest_mock::{
        Method, StatusCode, StubClient, StubDefault, StubSettings, StubStrictness, Url,
    };

    use serde_json::Value as JsonValue;

    #[test]
    fn check_limit_returns_err_above_hard_limit() {
        assert_eq!(
            check_limit(LIST_HARD_LIMIT + 1),
            Err(Error::AboveLimit(
                String::from("limit"),
                LIST_HARD_LIMIT + 1,
                LIST_HARD_LIMIT
            ))
        );

        assert_eq!(
            check_limit(LIST_HARD_LIMIT + 48613468),
            Err(Error::AboveLimit(
                String::from("limit"),
                LIST_HARD_LIMIT + 48613468,
                LIST_HARD_LIMIT
            ))
        );
    }

    #[test]
    fn check_limit_returns_ok_under_hard_limit() {
        assert!(check_limit(0).is_ok());
        assert!(check_limit(30).is_ok());
        assert!(check_limit(LIST_HARD_LIMIT).is_ok());
    }

    #[test]
    fn list_page() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let response = include_str!("mocked/index_fluffy_rating-s_limit-5_page-4.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<Post> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Post::try_from(v).unwrap())
            .collect();

        let request =
            Url::parse("https://e621.net/post/index.json?limit=5&page=4&tags=fluffy%20rating%3As")
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
            client.list_page(&["fluffy", "rating:s"][..], 5, 4),
            Ok(expected)
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

        let response = include_str!("mocked/index_fluffy_rating-s_limit-80_before-id-1866033.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<Post> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Post::try_from(v).unwrap())
            .collect();

        let request = Url::parse(
            "https://e621.net/post/index.json?limit=80&before_id=1866033&tags=fluffy%20rating%3As",
        )
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
            client.list_before(&["fluffy", "rating:s"][..], 80, 1866033),
            Ok(expected)
        );
    }

    #[test]
    fn list_above_limit_returns_err() {
        let client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        assert_eq!(
            client.list(&["fluffy", "rating:s"][..], 400),
            Err(Error::AboveLimit(
                String::from("limit"),
                400,
                LIST_HARD_LIMIT
            ))
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
                Url::parse("https://e621.net/post/index.json?limit=5&tags=fluffy%20rating%3As")
                    .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(client.list(&["fluffy", "rating:s"][..], 5), Ok(expected));
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

        assert_eq!(client.list(&["fluffy", "rating:s"][..], 5), Ok(expected));
    }

    #[test]
    fn search_above_limit() {
        let mut client = Client {
            headers: create_header_map(b"rs621/unit_test").unwrap(),
            client: StubClient::new(StubSettings {
                default: StubDefault::Error,
                strictness: StubStrictness::MethodUrl,
            }),
        };

        let first_expected_response = include_str!("mocked/index_fluffy_rating-s_limit-320.json");
        let second_expected_response =
            include_str!("mocked/index_fluffy_rating-s_limit-80_before-id-1866033.json");

        let first_request =
            Url::parse("https://e621.net/post/index.json?limit=320&tags=fluffy%20rating%3As")
                .unwrap();

        let second_request = Url::parse(
            "https://e621.net/post/index.json?limit=80&before_id=1866033&tags=fluffy%20rating%3As",
        )
        .unwrap();

        let expected: Vec<Post> = serde_json::from_str::<JsonValue>(first_expected_response)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .chain(
                serde_json::from_str::<JsonValue>(second_expected_response)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter(),
            )
            .map(|v| Post::try_from(v).unwrap())
            .collect();

        assert!(client
            .client
            .stub(first_request)
            .method(Method::GET)
            .response()
            .body(first_expected_response)
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(second_request)
            .method(Method::GET)
            .response()
            .body(second_expected_response)
            .mock()
            .is_ok());

        assert_eq!(
            client.search(&["fluffy", "rating:s"][..], 400),
            Ok(expected)
        );
    }

    #[test]
    fn search_no_result() {
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
                Url::parse("https://e621.net/post/index.json?limit=5&tags=fluffy%20rating%3As")
                    .unwrap()
            )
            .method(Method::GET)
            .response()
            .body(response)
            .mock()
            .is_ok());

        assert_eq!(client.search(&["fluffy", "rating:s"][..], 5), Ok(expected));
    }

    #[test]
    fn search_simple() {
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

        assert_eq!(client.search(&["fluffy", "rating:s"][..], 5), Ok(expected));
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
