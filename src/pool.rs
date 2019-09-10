use super::{
    client::Client,
    error::{Error, Result as Rs621Result},
    post::Post,
    utils::{get_json_api_time, get_json_value_as},
};
use chrono::{offset::Utc, DateTime};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

/// Iterator returning pools from a search query.
#[derive(Debug)]
pub struct PoolIter<'a, C: reqwest_mock::Client> {
    client: &'a Client<C>,
    query: Option<String>,

    page: u64,
    chunk: Vec<Rs621Result<PoolListResult<'a, C>>>,
    ended: bool,
}

impl<'a, C: reqwest_mock::Client> PoolIter<'a, C> {
    fn new(client: &'a Client<C>, query: Option<&str>) -> PoolIter<'a, C> {
        PoolIter {
            client,
            query: query.map(urlencoding::encode),

            page: 1,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl<'a, C: reqwest_mock::Client> Iterator for PoolIter<'a, C> {
    type Item = Rs621Result<PoolListResult<'a, C>>;

    fn next(&mut self) -> Option<Rs621Result<PoolListResult<'a, C>>> {
        // check if we need to load a new chunk of results
        if self.chunk.is_empty() {
            // get the JSON
            match self.client.get_json(&format!(
                "https://e621.net/pool/index.json?page={}{}",
                {
                    let page = self.page;
                    self.page += 1;
                    page
                },
                match &self.query {
                    None => String::new(),
                    Some(title) => format!("&query={}", title),
                }
            )) {
                Ok(body) => {
                    // put everything in the chunk
                    self.chunk = body
                        .as_array()
                        .unwrap()
                        .iter()
                        .rev()
                        .map(|v| PoolListResult::try_from((v, self.client)))
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
            // get a pool
            let pool = self.chunk.pop().unwrap();

            // return the pool
            Some(pool)
        } else {
            // pop any eventual error
            // Vec::pop returns None if the Vec is empty anyway
            self.chunk.pop()
        }
    }
}

/// Represents results returned by `pool/index.json`.
#[derive(Debug)]
pub struct PoolListResult<'a, C: reqwest_mock::Client> {
    client: &'a Client<C>,

    /// The raw JSON description of the pool list result (from the API).
    pub raw: String,

    /// The ID of the pool.
    pub id: u64,
    /// The name of the pool.
    pub name: String,
    /// When the pool was created.
    pub created_at: DateTime<Utc>,
    /// Last time the pool was updated.
    pub updated_at: DateTime<Utc>,
    /// The uploader's user ID.
    pub user_id: u64,
    /// Whether the pool is locked.
    pub is_locked: bool,
    /// How many posts the pool contains.
    pub post_count: u64,
}

impl<'a, C: reqwest_mock::Client> TryFrom<(&JsonValue, &'a Client<C>)> for PoolListResult<'a, C> {
    type Error = super::error::Error;

    fn try_from((v, client): (&JsonValue, &'a Client<C>)) -> Rs621Result<Self> {
        Ok(PoolListResult {
            client,

            raw: v.to_string(),

            id: get_json_value_as(&v, "id", JsonValue::as_u64)?,
            name: get_json_value_as(&v, "name", JsonValue::as_str)?.to_string(),
            user_id: v["user_id"].as_u64().unwrap(),
            created_at: get_json_api_time(&v, "created_at")?,
            updated_at: get_json_api_time(&v, "updated_at")?,
            is_locked: get_json_value_as(&v, "is_locked", JsonValue::as_bool)?,
            post_count: v["post_count"].as_u64().unwrap(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Pool {
    /// The raw JSON description of the pool (from the API).
    pub raw: String,

    /// The ID of the pool.
    pub id: u64,
    /// The name of the pool.
    pub name: String,
    /// The pool's description.
    pub description: String,
    /// The uploader's user ID.
    pub user_id: u64,
    /// When the pool was created.
    pub created_at: DateTime<Utc>,
    /// Last time the pool was updated.
    pub updated_at: DateTime<Utc>,
    /// Whether the pool is locked.
    pub is_locked: bool,
    /// Whether the pool is locked.
    pub is_active: bool,
    /// The posts this pool contains.
    pub posts: Vec<Post>,
}

impl TryFrom<&JsonValue> for Pool {
    type Error = super::error::Error;

    fn try_from(v: &JsonValue) -> Rs621Result<Self> {
        Ok(Pool {
            raw: v.to_string(),

            id: get_json_value_as(&v, "id", JsonValue::as_u64)?,
            name: get_json_value_as(&v, "name", JsonValue::as_str)?.to_string(),
            description: get_json_value_as(&v, "description", JsonValue::as_str)?.to_string(),
            user_id: v["user_id"].as_u64().unwrap(),
            created_at: get_json_api_time(&v, "created_at")?,
            updated_at: get_json_api_time(&v, "updated_at")?,
            is_locked: get_json_value_as(&v, "is_locked", JsonValue::as_bool)?,
            is_active: get_json_value_as(&v, "is_active", JsonValue::as_bool)?,
            posts: v["posts"]
                .as_array()
                .unwrap()
                .iter()
                .map(Post::try_from)
                .collect::<Rs621Result<Vec<Post>>>()?,
        })
    }
}

// An easy way to convert a PoolListResult into the corresponding regular Pool
// Currently just uses Client::get_pool with the id from the PoolListResult
impl<C: reqwest_mock::Client> TryFrom<PoolListResult<'_, C>> for Pool {
    type Error = Error;

    fn try_from(r: PoolListResult<'_, C>) -> Rs621Result<Pool> {
        let id = r.id;
        r.client.get_pool(id)
    }
}

impl<C: reqwest_mock::Client> Client<C> {
    /// Returns the pool with the given ID.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::pool::Pool;
    /// # fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let pool = client.get_pool(18274)?;
    ///
    /// assert_eq!(pool.id, 18274);
    /// # Ok(()) }
    /// ```
    ///
    /// This function performs a request; it will be subject to a short sleep time to ensure that
    /// the API rate limit isn't exceeded.
    pub fn get_pool(&self, id: u64) -> Rs621Result<Pool> {
        let body = self.get_json(&format!("https://e621.net/pool/show.json?id={}", id))?;

        Pool::try_from(&body)
    }

    /// Returns an iterator over all the pools in the website.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::pool::Pool;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let mut pools = client.pool_list();
    ///
    /// assert!(pools.next().unwrap().unwrap().id != 0);
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    pub fn pool_list<'a>(&'a self) -> PoolIter<'a, C> {
        PoolIter::new(self, None)
    }

    /// Search all the pools in the website and returns an iterator over the results.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::pool::Pool;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let mut pools = client.pool_search("test");
    ///
    /// assert!(pools.next().unwrap().unwrap().name.contains("test"));
    /// # Ok(()) }
    /// ```
    ///
    /// This function can perform a request; it might be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded.
    pub fn pool_search<'a>(&'a self, query: &str) -> PoolIter<'a, C> {
        PoolIter::new(self, Some(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::offset::TimeZone;
    use reqwest_mock::{Method, Url};

    #[test]
    fn pool_from_json() {
        let example_json = include_str!("mocked/pool_18274.json");

        let parsed = serde_json::from_str::<JsonValue>(example_json).unwrap();
        let pool = Pool::try_from(&parsed).unwrap();

        assert_eq!(pool.id, 18274);
        assert_eq!(pool.is_active, true);
        assert_eq!(pool.is_locked, false);
        assert_eq!(pool.name, "oBEARwatch_by_Murasaki_Yuri");
        assert_eq!(pool.posts.len(), 8);
        assert_eq!(pool.user_id, 357072);
        assert_eq!(pool.created_at, Utc.timestamp(1567963035, 63943000));
        assert_eq!(pool.updated_at, Utc.timestamp(1567964144, 960193000));
    }

    #[test]
    fn get_pool() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/show.json?id=18274").unwrap())
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/pool_18274.json"))
            .mock()
            .is_ok());

        let pool = client.get_pool(18274).unwrap();
        assert_eq!(pool.id, 18274);
    }

    #[test]
    fn pool_list() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/index.json?page=1").unwrap())
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/pool_list-page_1.json"))
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/index.json?page=2").unwrap())
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/pool_list-page_2.json"))
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/index.json?page=3").unwrap())
            .method(Method::GET)
            .response()
            .body("[]")
            .mock()
            .is_ok());

        let pools: Vec<_> = client.pool_list().collect();

        // We know how many pools we have because we've mocked the requests. Hah!
        assert_eq!(pools.len(), 6);
    }

    #[test]
    fn pool_search() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/index.json?page=1&query=foo").unwrap())
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/pool_search-foo.json"))
            .mock()
            .is_ok());

        assert!(client
            .client
            .stub(Url::parse("https://e621.net/pool/index.json?page=2&query=foo").unwrap())
            .method(Method::GET)
            .response()
            .body("[]")
            .mock()
            .is_ok());

        // Should all contain foo in the name
        for pool in client.pool_search("foo") {
            assert!(pool.unwrap().name.contains("foo"));
        }
    }
}
