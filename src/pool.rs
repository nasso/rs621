use super::{
    client::Client,
    error::Result as Rs621Result,
    post::Post,
    utils::{get_json_api_time, get_json_value_as},
};
use chrono::{offset::Utc, DateTime};
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use serde_json::Value as JsonValue;
use std::{convert::TryFrom, pin::Pin};

/// An iterator over [`PoolListEntry`]s.
///
/// [`PoolListEntry`]: struct.PoolListEntry.html
pub struct PoolStream<'a> {
    client: &'a Client,
    query: Option<String>,

    query_url: Option<String>,
    query_future: Option<Pin<Box<dyn Future<Output = Rs621Result<serde_json::Value>> + Send>>>,

    page: u64,
    chunk: Vec<Rs621Result<PoolListEntry>>,
    ended: bool,
}

impl<'a> PoolStream<'a> {
    fn new(client: &'a Client, query: Option<&str>) -> Self {
        PoolStream {
            client,
            query: query.map(urlencoding::encode),

            query_url: None,
            query_future: None,

            page: 1,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl Stream for PoolStream<'_> {
    type Item = Rs621Result<PoolListEntry>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Rs621Result<PoolListEntry>>> {
        let this = self.get_mut();

        // poll the pending query future if there's any
        if let Some(ref mut fut) = this.query_future {
            match fut.as_mut().poll(cx) {
                Poll::Ready(res) => {
                    // the future is finished, drop it
                    this.query_future = None;

                    match res {
                        Ok(body) => {
                            // put everything in the chunk
                            this.chunk = body
                                .as_array()
                                .unwrap()
                                .iter()
                                .rev()
                                .map(|v| PoolListEntry::try_from(v))
                                .collect();

                            // mark the stream as ended if there was no posts
                            this.ended = this.chunk.is_empty();
                        }

                        // if there was an error, stream it and mark the stream as ended
                        Err(e) => {
                            this.ended = true;
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }

                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }

        if this.ended {
            // the stream ended because:
            // 1. there was an error
            // 2. there's simply no more posts
            Poll::Ready(None)
        } else if this.chunk.is_empty() {
            // we need to load a new chunk of posts
            let url = format!(
                "/pool/index.json?page={}{}",
                {
                    let page = this.page;
                    this.page += 1;
                    page
                },
                match &this.query {
                    None => String::new(),
                    Some(title) => format!("&query={}", title),
                }
            );
            this.query_url = Some(url);

            // get the JSON
            this.query_future = Some(Box::pin(
                this.client
                    .get_json_endpoint(this.query_url.as_ref().unwrap()),
            ));

            // the stream is immediately pending for the new future
            Poll::Pending
        } else {
            // get a post
            let post = this.chunk.pop().unwrap();

            // stream the post
            Poll::Ready(Some(post))
        }
    }
}

/// Represents the pool information returned by pool listing functions.
///
/// The main difference between [`PoolListEntry`] and [`Pool`] is the absence of the description
/// field in the former.
/// You can convert a [`PoolListEntry`] to a regular [`Pool`] using a `&Client` because [`Pool`] is
/// `From<(PoolListEntry, &Client)>`:
///
/// ```no_run
/// # use rs621::client::Client;
/// # use rs621::pool::{Pool, PoolListEntry};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::convert::TryFrom;
///
/// let client = Client::new("MyProject/1.0 (by username on e621)")?;
///
/// let entry: PoolListEntry = client.pool_list().next().unwrap()?;
/// let pool = Pool::try_from((entry, &client))?;
///
/// println!("Description of pool #{}: {}", pool.id, pool.description);
/// # Ok(()) }
/// ```
/// _Note: This function performs a request; it will be subject to a short sleep time to ensure that
/// the API rate limit isn't exceeded._
///
/// [`Pool`]: struct.Pool.html
/// [`PoolListEntry`]: struct.PoolListEntry.html
#[derive(Debug)]
pub struct PoolListEntry {
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

impl TryFrom<&JsonValue> for PoolListEntry {
    type Error = super::error::Error;

    fn try_from(v: &JsonValue) -> Rs621Result<Self> {
        Ok(PoolListEntry {
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

/// Structure representing a pool.
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

impl Client {
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
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub async fn get_pool(&self, id: u64) -> Rs621Result<Pool> {
        let body = self
            .get_json_endpoint(&format!("/pool/show.json?id={}", id))
            .await?;

        Pool::try_from(&body)
    }

    /// Returns an iterator over all the pools on the website.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::pool::Pool;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for pool in client.pool_list().take(3) {
    ///     assert!(pool?.id != 0);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// The iterator returns [`PoolListEntry`]s, which you can convert to regular [`Pool`]s because
    /// [`Pool`] is `From<(PoolListEntry, &Client)>`. See [`PoolListEntry`].
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    ///
    /// [`Pool`]: ../pool/struct.Pool.html
    /// [`PoolListEntry`]: ../pool/struct.PoolListEntry.html
    pub fn pool_list<'a>(&'a self) -> PoolStream<'a> {
        PoolStream::new(self, None)
    }

    /// Search all the pools in the website and returns an iterator over the results.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::pool::Pool;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for pool in client.pool_search("foo").take(3) {
    ///     assert!(pool?.name.contains("foo"));
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// The iterator returns [`PoolListEntry`]s, which you can convert to regular [`Pool`]s because
    /// [`Pool`] is `From<(PoolListEntry, &Client)>`. See [`PoolListEntry`].
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    ///
    /// [`Pool`]: ../pool/struct.Pool.html
    /// [`PoolListEntry`]: ../pool/struct.PoolListEntry.html
    pub fn pool_search<'a>(&'a self, query: &str) -> PoolStream<'a> {
        PoolStream::new(self, Some(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::offset::TimeZone;
    use mockito::mock;

    #[tokio::test]
    async fn pool_list_result_from_json() {
        let example_json = include_str!("mocked/pool_list_result-12668.json");

        let parsed = serde_json::from_str::<JsonValue>(example_json).unwrap();
        let result = PoolListEntry::try_from(&parsed).unwrap();

        assert_eq!(result.id, 12668);
        assert_eq!(result.is_locked, false);
        assert_eq!(result.name, "Random SFW name");
        assert_eq!(result.post_count, 33);
        assert_eq!(result.user_id, 171621);
        assert_eq!(result.created_at, Utc.timestamp(1506450220, 569794000));
        assert_eq!(result.updated_at, Utc.timestamp(1568077422, 207421000));
    }

    #[tokio::test]
    async fn pool_from_json() {
        let example_json = include_str!("mocked/pool_18274.json");

        let parsed = serde_json::from_str::<JsonValue>(example_json).unwrap();
        let pool = Pool::try_from(&parsed).unwrap();

        assert_eq!(pool.id, 18274);
        assert_eq!(pool.is_active, true);
        assert_eq!(pool.is_locked, false);
        assert_eq!(pool.name, "oBEARwatch_by_Murasaki_Yuri");
        assert_eq!(pool.description, "");
        assert_eq!(pool.posts.len(), 8);
        assert_eq!(pool.user_id, 357072);
        assert_eq!(pool.created_at, Utc.timestamp(1567963035, 63943000));
        assert_eq!(pool.updated_at, Utc.timestamp(1567964144, 960193000));
    }

    #[tokio::test]
    async fn get_pool() {
        let client = Client::new(b"rs621/unit_test").unwrap();

        let _m = mock("GET", "/pool/show.json?id=18274")
            .with_body(include_str!("mocked/pool_18274.json"))
            .create();

        let pool = client.get_pool(18274).await.unwrap();
        assert_eq!(pool.id, 18274);
    }

    #[tokio::test]
    async fn pool_list() {
        let client = Client::new(b"rs621/unit_test").unwrap();

        let _m = [
            mock("GET", "/pool/index.json?page=1")
                .with_body(include_str!("mocked/pool_list-page_1.json"))
                .create(),
            mock("GET", "/pool/index.json?page=2")
                .with_body(include_str!("mocked/pool_list-page_2.json"))
                .create(),
            // have the next page be empty to end the iterator
            mock("GET", "/pool/index.json?page=3")
                .with_body("[]")
                .create(),
        ];

        let pools: Vec<_> = client.pool_list().collect();

        // We know how many pools we have because we've mocked the requests. Hah!
        assert_eq!(pools.len(), 6);
    }

    #[tokio::test]
    async fn pool_search() {
        let client = Client::new(b"rs621/unit_test").unwrap();

        let _m = [
            mock("GET", "/pool/index.json?page=1&query=foo")
                .with_body(include_str!("mocked/pool_search-foo.json"))
                .create(),
            // have the next page be empty to end the iterator
            mock("GET", "/pool/index.json?page=2&query=foo")
                .with_body("[]")
                .create(),
        ];

        // Should all contain foo in the name
        for pool in client.pool_search("foo") {
            assert!(pool.unwrap().name.contains("foo"));
        }
    }
}
