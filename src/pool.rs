use crate::error::Error;

use {
    super::{client::Client, error::Result as Rs621Result},
    chrono::{offset::Utc, DateTime},
    derivative::Derivative,
    futures::{
        prelude::*,
        task::{Context, Poll},
    },
    itertools::Itertools,
    serde::Deserialize,
    std::pin::Pin,
};

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolCategory {
    Series,
    Collection,
}

/// Structure representing a pool.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Pool {
    pub id: u64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub creator_id: u64,
    pub description: String,
    pub is_active: bool,
    pub category: PoolCategory,
    pub is_deleted: bool,
    pub post_ids: Vec<u64>,
    pub creator_name: String,
    pub post_count: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PoolSearchOrder {
    Name,
    CreatedAt,
    UpdatedAt,
    PostCount,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct PoolSearch {
    pub name_matches: Option<String>,
    pub id: Option<Vec<u64>>,
    pub description_matches: Option<String>,
    pub creator_name: Option<String>,
    pub creator_id: Option<u64>,
    pub is_active: Option<bool>,
    pub is_deleted: Option<bool>,
    pub category: Option<PoolCategory>,
    pub order: Option<PoolSearchOrder>,
}

impl PoolSearch {
    fn to_search_parameters(&self) -> String {
        let mut params = String::new();

        if let Some(ref value) = self.name_matches {
            params.push('&');
            params.push_str(&urlencoding::encode("search[name_matches]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value));
        }

        if let Some(ref value) = self.id {
            params.push('&');
            params.push_str(&urlencoding::encode("search[id]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value.iter().join(",")));
        }

        if let Some(ref value) = self.description_matches {
            params.push('&');
            params.push_str(&urlencoding::encode("search[description_matches]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value));
        }

        if let Some(ref value) = self.creator_name {
            params.push('&');
            params.push_str(&urlencoding::encode("search[creator_name]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value));
        }

        if let Some(ref value) = self.creator_id {
            params.push('&');
            params.push_str(&urlencoding::encode("search[creator_id]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value.to_string()));
        }

        if let Some(ref value) = self.is_active {
            params.push('&');
            params.push_str(&urlencoding::encode("search[is_active]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value.to_string()));
        }

        if let Some(ref value) = self.is_deleted {
            params.push('&');
            params.push_str(&urlencoding::encode("search[is_deleted]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(&value.to_string()));
        }

        if let Some(ref value) = self.category {
            params.push('&');
            params.push_str(&urlencoding::encode("search[category]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(match value {
                PoolCategory::Series => "series",
                PoolCategory::Collection => "collection",
            }));
        }

        if let Some(ref value) = self.order {
            params.push('&');
            params.push_str(&urlencoding::encode("search[order]"));
            params.push_str("=");
            params.push_str(&urlencoding::encode(match value {
                PoolSearchOrder::Name => "name",
                PoolSearchOrder::CreatedAt => "created_at",
                PoolSearchOrder::UpdatedAt => "updated_at",
                PoolSearchOrder::PostCount => "post_count",
            }));
        }

        params
    }

    pub fn new() -> Self {
        PoolSearch::default()
    }

    pub fn name_matches<T: ToString>(mut self, value: T) -> Self {
        self.name_matches = Some(value.to_string());
        self
    }

    pub fn id(mut self, value: Vec<u64>) -> Self {
        self.id = Some(value);
        self
    }

    pub fn description_matches<T: ToString>(mut self, value: T) -> Self {
        self.description_matches = Some(value.to_string());
        self
    }

    pub fn creator_name<T: ToString>(mut self, value: T) -> Self {
        self.creator_name = Some(value.to_string());
        self
    }

    pub fn creator_id(mut self, value: u64) -> Self {
        self.creator_id = Some(value);
        self
    }

    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    pub fn is_deleted(mut self, value: bool) -> Self {
        self.is_deleted = Some(value);
        self
    }

    pub fn category(mut self, value: PoolCategory) -> Self {
        self.category = Some(value);
        self
    }

    pub fn order(mut self, value: PoolSearchOrder) -> Self {
        self.order = Some(value);
        self
    }
}

type PoolSearchApiResponse = Vec<Pool>;

/// A stream of [`Pool`]s.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PoolStream<'a> {
    client: &'a Client,
    search: PoolSearch,

    query_url: Option<String>,
    #[derivative(Debug = "ignore")]
    query_future: Option<Pin<Box<dyn Future<Output = Rs621Result<serde_json::Value>> + Send>>>,

    page: u64,
    chunk: Vec<Rs621Result<Pool>>,
    ended: bool,
}

impl<'a> PoolStream<'a> {
    fn new(client: &'a Client, search: PoolSearch) -> Self {
        PoolStream {
            client,
            search,

            query_url: None,
            query_future: None,

            page: 1,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl<'a> Stream for PoolStream<'a> {
    type Item = Rs621Result<Pool>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Rs621Result<Pool>>> {
        enum QueryPollRes {
            Pending,
            Err(crate::error::Error),
            NotFetching,
        }

        let this = self.get_mut();

        loop {
            // poll the pending query future if there's any
            let query_status = if let Some(ref mut fut) = this.query_future {
                match fut.as_mut().poll(cx) {
                    Poll::Ready(res) => {
                        // the future is finished, drop it
                        this.query_future = None;

                        match res {
                            Ok(body) => {
                                // put everything in the chunk
                                this.chunk =
                                    match serde_json::from_value::<PoolSearchApiResponse>(body) {
                                        Ok(res) => {
                                            res.into_iter().rev().map(|pool| Ok(pool)).collect()
                                        }
                                        Err(e) => vec![Err(Error::Serial(format!("{}", e)))],
                                    };

                                // mark the stream as ended if there was no pools
                                this.ended = this.chunk.is_empty();
                                QueryPollRes::NotFetching
                            }

                            // if there was an error, stream it and mark the stream as ended
                            Err(e) => {
                                this.ended = true;
                                QueryPollRes::Err(e)
                            }
                        }
                    }

                    Poll::Pending => QueryPollRes::Pending,
                }
            } else {
                QueryPollRes::NotFetching
            };

            match query_status {
                QueryPollRes::Err(e) => return Poll::Ready(Some(Err(e))),
                QueryPollRes::Pending => return Poll::Pending,
                QueryPollRes::NotFetching if this.ended => {
                    // the stream ended because:
                    // 1. there was an error
                    // 2. there's simply no more elements
                    return Poll::Ready(None);
                }
                QueryPollRes::NotFetching if !this.chunk.is_empty() => {
                    // get a post
                    let pool = this.chunk.pop().unwrap();

                    // stream the post
                    return Poll::Ready(Some(pool));
                }
                QueryPollRes::NotFetching => {
                    // we need to load a new chunk of pools
                    let url = format!(
                        "/pools.json?page={}{}",
                        {
                            let page = this.page;
                            this.page += 1;
                            page
                        },
                        this.search.to_search_parameters(),
                    );
                    this.query_url = Some(url);

                    // get the JSON
                    this.query_future = Some(Box::pin(
                        this.client
                            .get_json_endpoint(this.query_url.as_ref().unwrap()),
                    ));
                }
            }
        }
    }
}

impl Client {
    /// Performs a pool search.
    ///
    /// ```no_run
    /// # use rs621::{client::Client, pool::{Pool, PoolSearch}};
    /// use futures::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    ///
    /// let mut pool_stream = client.pool_search(PoolSearch::new().name_matches("foo"));
    ///
    /// while let Some(pool) = pool_stream.next().await {
    ///     assert!(pool?.name.contains("foo"));
    /// }
    /// # Ok(()) }
    /// ```
    pub fn pool_search<'a>(&'a self, search: PoolSearch) -> PoolStream<'a> {
        PoolStream::new(self, search)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[tokio::test]
    async fn pool_search() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let expected: Vec<Rs621Result<Pool>> = serde_json::from_str::<PoolSearchApiResponse>(
            include_str!("mocked/pool_search-foo.json"),
        )
        .unwrap()
        .into_iter()
        .map(|x| Ok(x))
        .collect();

        let _m = [
            mock("GET", "/pools.json?page=1&search%5Bname_matches%5D=foo")
                .with_body(include_str!("mocked/pool_search-foo.json"))
                .create(),
            // have the next page be empty to end the iterator
            mock("GET", "/pools.json?page=2&search%5Bname_matches%5D=foo")
                .with_body("[]")
                .create(),
        ];

        // Should all contain foo in the name
        let pools: Vec<Rs621Result<Pool>> = client
            .pool_search(PoolSearch::new().name_matches("foo"))
            .collect()
            .await;

        assert_eq!(pools, expected);
    }
}
