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
    serde::{
        de::{self, Visitor},
        Deserialize, Deserializer,
    },
    std::{borrow::Borrow, pin::Pin},
};

/// Chunk size used for iterators performing requests
const ITER_CHUNK_SIZE: u64 = 320;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum PostFileExtension {
    #[serde(rename = "jpg")]
    Jpeg,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "gif")]
    Gif,
    #[serde(rename = "swf")]
    Swf,
    #[serde(rename = "webm")]
    WebM,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostFile {
    pub width: u64,
    pub height: u64,
    pub ext: PostFileExtension,
    pub size: u64,
    pub md5: String,
    pub url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostPreview {
    pub width: u64,
    pub height: u64,
    pub url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostSample {
    pub width: u64,
    pub height: u64,
    pub url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostScore {
    pub up: i64,
    pub down: i64,
    pub total: i64,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostTags {
    pub general: Vec<String>,
    pub species: Vec<String>,
    pub character: Vec<String>,
    pub artist: Vec<String>,
    pub invalid: Vec<String>,
    pub lore: Vec<String>,
    pub meta: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostFlags {
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub pending: bool,
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub flagged: bool,
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub note_locked: bool,
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub status_locked: bool,
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub rating_locked: bool,
    #[serde(deserialize_with = "nullable_bool_from_json")]
    pub deleted: bool,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum PostRating {
    #[serde(rename = "s")]
    Safe,
    #[serde(rename = "q")]
    Questionable,
    #[serde(rename = "e")]
    Explicit,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostRelationships {
    pub parent_id: Option<u64>,
    pub has_children: bool,
    pub has_active_children: bool,
    pub children: Vec<u64>,
}

/// Structure representing a post.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Post {
    pub id: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub file: PostFile,
    pub preview: PostPreview,
    pub sample: Option<PostSample>,
    pub score: PostScore,
    pub tags: PostTags,
    pub locked_tags: Vec<String>,
    pub change_seq: u64,
    pub flags: PostFlags,
    pub rating: PostRating,
    pub fav_count: u64,
    pub sources: Vec<String>,
    pub pools: Vec<u64>,
    pub relationships: PostRelationships,
    pub approver_id: Option<u64>,
    pub uploader_id: u64,
    pub description: String,
    pub comment_count: u64,
    pub is_favorited: bool,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct PostListApiResponse {
    pub posts: Vec<Post>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct PostShowApiResponse {
    pub post: Post,
}

fn nullable_bool_from_json<'de, D>(de: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct NullableBoolVisitor;

    impl<'de> Visitor<'de> for NullableBoolVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("null or bool")
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }

        fn visit_unit<E: de::Error>(self) -> Result<bool, E> {
            Ok(false)
        }
    }

    de.deserialize_any(NullableBoolVisitor)
}

/// A search query. Contains information about the tags used and an URL encoded version of the tags.
#[derive(Debug, PartialEq, Clone)]
pub struct Query {
    url_encoded_tags: String,
    ordered: bool,
}

impl<T> From<&[T]> for Query
where
    T: AsRef<str>,
{
    fn from(q: &[T]) -> Self {
        let tags: Vec<&str> = q.iter().map(|t| t.as_ref()).collect();
        let query_str = tags.join(" ");
        let url_encoded_tags = urlencoding::encode(&query_str);
        let ordered = tags.iter().any(|t| t.starts_with("order:"));

        Query {
            url_encoded_tags,
            ordered,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SearchPage {
    Page(u64),
    BeforePost(u64),
    AfterPost(u64),
}

/// Iterator returning posts from a search query.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PostSearchStream<'a> {
    client: &'a Client,
    query: Query,

    query_url: Option<String>,

    #[derivative(Debug = "ignore")]
    query_future: Option<Pin<Box<dyn Future<Output = Rs621Result<serde_json::Value>> + Send>>>,

    next_page: SearchPage,
    chunk: Vec<Rs621Result<Post>>,
    ended: bool,
}

impl<'a> PostSearchStream<'a> {
    fn new<T: Into<Query>>(client: &'a Client, query: T, page: SearchPage) -> Self {
        PostSearchStream {
            client: client,
            query: query.into(),

            query_url: None,
            query_future: None,

            next_page: page,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl<'a> Stream for PostSearchStream<'a> {
    type Item = Rs621Result<Post>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Rs621Result<Post>>> {
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
                                    match serde_json::from_value::<PostListApiResponse>(body) {
                                        Ok(res) => res
                                            .posts
                                            .into_iter()
                                            .rev()
                                            .map(|post| Ok(post))
                                            .collect(),
                                        Err(e) => vec![Err(Error::Serial(format!("{}", e)))],
                                    };

                                let last_id = match this.chunk.first() {
                                    Some(Ok(post)) => post.id,
                                    _ => 0,
                                };

                                // we now know what will be the next page
                                this.next_page = if this.query.ordered {
                                    match this.next_page {
                                        SearchPage::Page(i) => SearchPage::Page(i + 1),
                                        _ => SearchPage::Page(1),
                                    }
                                } else {
                                    match this.next_page {
                                        SearchPage::Page(_) => SearchPage::BeforePost(last_id),
                                        SearchPage::BeforePost(_) => {
                                            SearchPage::BeforePost(last_id)
                                        }
                                        SearchPage::AfterPost(_) => SearchPage::AfterPost(last_id),
                                    }
                                };

                                // mark the stream as ended if there was no posts
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
                    let post = this.chunk.pop().unwrap();

                    // stream the post
                    return Poll::Ready(Some(post));
                }
                QueryPollRes::NotFetching => {
                    // we need to load a new chunk of posts
                    let url = format!(
                        "/posts.json?limit={}&page={}&tags={}",
                        ITER_CHUNK_SIZE,
                        match this.next_page {
                            SearchPage::Page(i) => format!("{}", i),
                            SearchPage::BeforePost(i) => format!("b{}", i),
                            SearchPage::AfterPost(i) => format!("a{}", i),
                        },
                        this.query.url_encoded_tags
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

/// Iterator returning posts from a search query.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PostStream<'a, I, T>
where
    T: Borrow<u64> + Unpin,
    I: Iterator<Item = T> + Unpin,
{
    client: &'a Client,
    ids: I,

    query_url: Option<String>,

    #[derivative(Debug = "ignore")]
    query_future: Option<Pin<Box<dyn Future<Output = Rs621Result<serde_json::Value>> + Send>>>,

    chunk: Vec<Rs621Result<Post>>,
}

impl<'a, I, T> PostStream<'a, I, T>
where
    T: Borrow<u64> + Unpin,
    I: Iterator<Item = T> + Unpin,
{
    fn new(client: &'a Client, ids: I) -> Self {
        PostStream {
            client,
            ids,
            query_url: None,
            query_future: None,
            chunk: Vec::new(),
        }
    }
}

impl<'a, I, T> Stream for PostStream<'a, I, T>
where
    T: Borrow<u64> + Unpin,
    I: Iterator<Item = T> + Unpin,
{
    type Item = Rs621Result<Post>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Rs621Result<Post>>> {
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
                                    match serde_json::from_value::<PostListApiResponse>(body) {
                                        Ok(res) => res
                                            .posts
                                            .into_iter()
                                            .rev()
                                            .map(|post| Ok(post))
                                            .collect(),
                                        Err(e) => vec![Err(Error::Serial(format!("{}", e)))],
                                    };

                                QueryPollRes::NotFetching
                            }

                            // if there was an error, stream it
                            Err(e) => QueryPollRes::Err(e),
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
                QueryPollRes::NotFetching if !this.chunk.is_empty() => {
                    // get a post
                    let post = this.chunk.pop().unwrap();

                    // stream the post
                    return Poll::Ready(Some(post));
                }
                QueryPollRes::NotFetching => {
                    // we need to load a new chunk of posts
                    let id_list = this.ids.by_ref().take(100).map(|x| *x.borrow()).join(",");

                    if id_list.is_empty() {
                        // the stream ended
                        return Poll::Ready(None);
                    }

                    let url = format!("/posts.json?tags=id%3A{}", id_list);
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
    /// Returns posts with the given IDs. Note that the order is NOT preserved!
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// use futures::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    /// let mut post_stream = client.get_posts(&[8595, 535, 2105, 1470]);
    ///
    /// while let Some(post) = post_stream.next().await {
    ///     println!("Post #{}", post?.id);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn get_posts<'a, I, J, T>(&'a self, ids: I) -> PostStream<'a, J, T>
    where
        T: Borrow<u64> + Unpin,
        J: Iterator<Item = T> + Unpin,
        I: IntoIterator<Item = T, IntoIter = J> + Unpin,
    {
        PostStream::new(self, ids.into_iter())
    }

    /// Returns a Stream over all the posts matching the search query.
    ///
    /// ```no_run
    /// # use rs621::{client::Client, post::PostRating};
    /// use futures::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    ///
    /// let mut post_stream = client.post_search(&["fluffy", "rating:s"][..]).take(3);
    ///
    /// while let Some(post) = post_stream.next().await {
    ///     assert_eq!(post?.rating, PostRating::Safe);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn post_search<'a, T: Into<Query>>(&'a self, tags: T) -> PostSearchStream<'a> {
        self.post_search_from_page(tags, SearchPage::Page(1))
    }

    /// Returns a Stream over all the posts matching the search query, starting from the given page.
    ///
    /// ```no_run
    /// # use {
    /// #     rs621::{client::Client, post::PostRating},
    /// #     futures::prelude::*,
    /// # };
    /// use rs621::post::SearchPage;
    /// # #[tokio::main]
    /// # async fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    ///
    /// let mut post_stream = client
    ///     .post_search_from_page(&["fluffy", "rating:s"][..], SearchPage::BeforePost(123456))
    ///     .take(3);
    ///
    /// while let Some(post) = post_stream.next().await {
    ///     let post = post?;
    ///     assert!(post.id < 123456);
    ///     assert_eq!(post.rating, PostRating::Safe);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn post_search_from_page<'a, T: Into<Query>>(
        &'a self,
        tags: T,
        page: SearchPage,
    ) -> PostSearchStream<'a> {
        PostSearchStream::new(self, tags, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, Matcher};

    #[tokio::test]
    async fn search_ordered() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s", "order:score"][..]);

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/posts.json?limit={}&page=1&tags={}",
                ITER_CHUNK_SIZE, query.url_encoded_tags
            )),
        )
        .with_body(include_str!(
            "mocked/320_page-1_fluffy_rating-s_order-score.json"
        ))
        .create();

        assert_eq!(
            client
                .post_search(query)
                .take(100)
                .collect::<Vec<_>>()
                .await,
            serde_json::from_str::<PostListApiResponse>(include_str!(
                "mocked/320_page-1_fluffy_rating-s_order-score.json"
            ))
            .unwrap()
            .posts
            .into_iter()
            .take(100)
            .map(|x| Ok(x))
            .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn search_above_limit_ordered() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s", "order:score"][..]);
        const PAGES: [&str; 2] = [
            include_str!("mocked/320_page-1_fluffy_rating-s_order-score.json"),
            include_str!("mocked/320_page-2_fluffy_rating-s_order-score.json"),
        ];

        let _m = [
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/posts.json?limit={}&page=1&tags={}",
                    ITER_CHUNK_SIZE, query.url_encoded_tags
                )),
            )
            .with_body(PAGES[0])
            .create(),
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/posts.json?limit={}&page=2&tags={}",
                    ITER_CHUNK_SIZE, query.url_encoded_tags
                )),
            )
            .with_body(PAGES[1])
            .create(),
        ];

        assert_eq!(
            client
                .post_search(query)
                .take(400)
                .collect::<Vec<_>>()
                .await,
            serde_json::from_str::<PostListApiResponse>(PAGES[0])
                .unwrap()
                .posts
                .into_iter()
                .chain(
                    serde_json::from_str::<PostListApiResponse>(PAGES[1])
                        .unwrap()
                        .posts
                        .into_iter()
                )
                .take(400)
                .map(|x| Ok(x))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn search_before_id() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s"][..]);
        let response_json = include_str!("mocked/320_fluffy_rating-s_before-2269211.json");
        let response: PostListApiResponse = serde_json::from_str(response_json).unwrap();
        let expected: Vec<_> = response.posts.into_iter().take(80).map(|x| Ok(x)).collect();

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/posts.json?limit={}&page=b2269211&tags={}",
                ITER_CHUNK_SIZE, query.url_encoded_tags
            )),
        )
        .with_body(response_json)
        .create();

        assert_eq!(
            client
                .post_search_from_page(query, SearchPage::BeforePost(2269211))
                .take(80)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn search_above_limit() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s"][..]);
        let responses_json: [&str; 2] = [
            include_str!("mocked/320_fluffy_rating-s.json"),
            include_str!("mocked/320_fluffy_rating-s_before-2269211.json"),
        ];
        let mut responses: [Option<PostListApiResponse>; 2] = [
            Some(serde_json::from_str(responses_json[0]).unwrap()),
            Some(serde_json::from_str(responses_json[1]).unwrap()),
        ];
        let expected: Vec<_> = responses[0]
            .take()
            .unwrap()
            .posts
            .into_iter()
            .chain(responses[1].take().unwrap().posts.into_iter())
            .take(400)
            .map(|x| Ok(x))
            .collect();

        let _m = [
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/posts.json?limit={}&page=1&tags={}",
                    ITER_CHUNK_SIZE, query.url_encoded_tags
                )),
            )
            .with_body(responses_json[0])
            .create(),
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/posts.json?limit={}&page=b2269211&tags={}",
                    ITER_CHUNK_SIZE, query.url_encoded_tags
                )),
            )
            .with_body(responses_json[1])
            .create(),
        ];

        assert_eq!(
            client
                .post_search(query)
                .take(400)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn search_no_result() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s"][..]);
        let response = "{\"posts\":[]}";

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/posts.json?limit={}&page=1&tags={}",
                ITER_CHUNK_SIZE, query.url_encoded_tags
            )),
        )
        .with_body(response)
        .create();

        assert_eq!(
            client.post_search(query).take(5).collect::<Vec<_>>().await,
            vec![]
        );
    }

    #[tokio::test]
    async fn search_simple() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::from(&["fluffy", "rating:s"][..]);
        let response_json = include_str!("mocked/320_fluffy_rating-s.json");
        let response: PostListApiResponse = serde_json::from_str(response_json).unwrap();
        let expected: Vec<_> = response.posts.into_iter().take(5).map(|x| Ok(x)).collect();

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/posts.json?limit={}&page=1&tags={}",
                ITER_CHUNK_SIZE, query.url_encoded_tags
            )),
        )
        .with_body(response_json)
        .create();

        assert_eq!(
            client.post_search(query).take(5).collect::<Vec<_>>().await,
            expected
        );
    }

    #[tokio::test]
    async fn get_posts_by_id() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response_json = include_str!("mocked/id_8595_535_2105_1470.json");
        let response: PostListApiResponse = serde_json::from_str(response_json).unwrap();
        let expected = response.posts;

        let _m = mock("GET", "/posts.json?tags=id%3A8595,535,2105,1470")
            .with_body(response_json)
            .create();

        assert_eq!(
            client
                .get_posts(&[8595, 535, 2105, 1470])
                .collect::<Vec<_>>()
                .await,
            expected.into_iter().map(|p| Ok(p)).collect::<Vec<_>>(),
        );
    }
}
