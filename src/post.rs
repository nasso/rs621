use super::{
    client::Client,
    error::Result as Rs621Result,
    utils::{get_json_api_time, get_json_value_as},
};
use chrono::{offset::Utc, DateTime, TimeZone};
use derivative::Derivative;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use serde_json::Value as JsonValue;
use std::{convert::TryFrom, fmt, pin::Pin};

/// Chunk size used for iterators performing requests
const ITER_CHUNK_SIZE: u64 = 320;

/// A search query. Contains information about the tags used and an URL encoded version of the tags.
#[derive(Debug, PartialEq, Clone)]
pub struct Query {
    str_url: String,
    ordered: bool,
}

impl From<&[&str]> for Query {
    fn from(q: &[&str]) -> Self {
        let query_str = q.join(" ");
        let str_url = urlencoding::encode(&query_str);
        let ordered = q.iter().any(|t| t.starts_with("order:"));

        Query { str_url, ordered }
    }
}

/// Iterator returning posts from a search query.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PostStream<'a> {
    client: &'a Client,
    query: Option<Query>,

    query_url: Option<String>,

    #[derivative(Debug = "ignore")]
    query_future: Option<Pin<Box<dyn Future<Output = Rs621Result<serde_json::Value>> + Send>>>,

    last_id: Option<u64>,
    page: u64,
    chunk: Vec<Rs621Result<Post>>,
    ended: bool,
}

impl<'a> PostStream<'a> {
    fn new<T: Into<Query>>(client: &'a Client, query: Option<T>, start_id: Option<u64>) -> Self {
        PostStream {
            client: client,
            query: query.map(T::into),

            query_url: None,
            query_future: None,

            last_id: start_id,
            page: 0,
            chunk: Vec::new(),
            ended: false,
        }
    }
}

impl<'a> Stream for PostStream<'a> {
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
                                this.chunk = body
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .rev()
                                    .map(Post::try_from)
                                    .collect();

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

                    // if there's an actual post, then it's the new last_id
                    if let Ok(ref p) = post {
                        this.last_id = Some(p.id);
                    }

                    // stream the post
                    return Poll::Ready(Some(post));
                }
                QueryPollRes::NotFetching => {
                    // we need to load a new chunk of posts
                    let url = format!(
                        "/post/index.json?limit={}{}{}",
                        ITER_CHUNK_SIZE,
                        if let Some(Query { ordered: true, .. }) = this.query {
                            this.page += 1;
                            format!("&page={}", this.page)
                        } else {
                            match this.last_id {
                                Some(i) => format!("&before_id={}", i),
                                None => String::new(),
                            }
                        },
                        match &this.query {
                            Some(q) => format!("&tags={}", q.str_url),
                            None => String::new(),
                        }
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

/// Post status.
#[derive(Debug, PartialEq, Eq)]
pub enum PostStatus {
    Active,
    Flagged,
    Pending,
    /// The `String` is the reason the post was deleted.
    Deleted(String),
}

impl PostStatus {
    /// Returns `true` if this `PostStatus` matches `PostStatus::Deleted(_)`.
    pub fn is_deleted(&self) -> bool {
        match self {
            PostStatus::Deleted(_) => true,
            _ => false,
        }
    }
}

impl TryFrom<(&str, Option<&str>)> for PostStatus {
    type Error = ();

    fn try_from(v: (&str, Option<&str>)) -> Result<PostStatus, ()> {
        match v.0 {
            "active" => Ok(PostStatus::Active),
            "flagged" => Ok(PostStatus::Flagged),
            "pending" => Ok(PostStatus::Pending),
            "deleted" => Ok(PostStatus::Deleted(
                v.1.map(String::from).unwrap_or_else(String::new),
            )),
            _ => Err(()),
        }
    }
}

impl TryFrom<(&JsonValue, Option<&str>)> for PostStatus {
    type Error = ();

    fn try_from(v: (&JsonValue, Option<&str>)) -> Result<PostStatus, ()> {
        match v.0.as_str() {
            Some(s) => PostStatus::try_from((s, v.1)),
            None => Err(()),
        }
    }
}

impl Default for PostStatus {
    fn default() -> PostStatus {
        PostStatus::Pending
    }
}

/// Post rating.
#[derive(Debug, PartialEq, Eq)]
pub enum PostRating {
    /// Safe For Work
    Safe,
    /// Wouldn't Recommend For Work
    Questionable,
    /// Not Safe For Work
    Explicit,
}

impl Default for PostRating {
    fn default() -> PostRating {
        // A default value doesn't make much sense here
        PostRating::Explicit
    }
}

impl TryFrom<&str> for PostRating {
    type Error = ();

    fn try_from(v: &str) -> Result<PostRating, ()> {
        match v {
            "s" => Ok(PostRating::Safe),
            "q" => Ok(PostRating::Questionable),
            "e" => Ok(PostRating::Explicit),
            _ => Err(()),
        }
    }
}

impl TryFrom<&JsonValue> for PostRating {
    type Error = ();

    fn try_from(v: &JsonValue) -> Result<PostRating, ()> {
        match v.as_str() {
            Some(s) => PostRating::try_from(s),
            None => Err(()),
        }
    }
}

impl fmt::Display for PostRating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PostRating::Explicit => write!(f, "explicit"),
            PostRating::Questionable => write!(f, "questionable"),
            PostRating::Safe => write!(f, "safe"),
        }
    }
}

/// Post file formats/extensions.
#[derive(Debug, PartialEq, Eq)]
pub enum PostFormat {
    /// Joint Photographic Experts Group image file format.
    JPG,
    /// Portable Network Graphics image file format.
    PNG,
    /// Graphics Interchange Format image file format (possibly animated).
    GIF,
    /// ShockWave Flash file format.
    SWF,
    /// WebM video file format.
    WEBM,
}

impl TryFrom<&str> for PostFormat {
    type Error = ();

    fn try_from(v: &str) -> Result<PostFormat, ()> {
        match v {
            "jpg" => Ok(PostFormat::JPG),
            "png" => Ok(PostFormat::PNG),
            "gif" => Ok(PostFormat::GIF),
            "swf" => Ok(PostFormat::SWF),
            "webm" => Ok(PostFormat::WEBM),
            _ => Err(()),
        }
    }
}

impl TryFrom<&JsonValue> for PostFormat {
    type Error = ();

    fn try_from(v: &JsonValue) -> Result<PostFormat, ()> {
        match v.as_str() {
            Some(s) => PostFormat::try_from(s),
            None => Err(()),
        }
    }
}

impl fmt::Display for PostFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PostFormat::JPG => write!(f, "JPG"),
            PostFormat::PNG => write!(f, "PNG"),
            PostFormat::GIF => write!(f, "GIF"),
            PostFormat::SWF => write!(f, "SWF"),
            PostFormat::WEBM => write!(f, "WEBM"),
        }
    }
}

/// Structure representing a post.
#[derive(Debug, PartialEq, Eq)]
pub struct Post {
    /// The raw JSON description of the post (from the API).
    pub raw: String,

    /// The ID of the post.
    pub id: u64,
    /// The post's MD5 hash. Unavailable for deleted posts.
    pub md5: Option<String>,
    /// The status of the post.
    pub status: PostStatus,

    /// Username of the user who uploaded the post.
    pub author: String,
    /// User ID of the user who uploaded the post. `None` if unknown.
    pub creator_id: Option<u64>,
    /// When the post was uploaded.
    pub created_at: DateTime<Utc>,

    /// A list of the post's artist tag(s).
    pub artists: Vec<String>,
    /// The post's tags.
    pub tags: Vec<String>,
    /// The post's rating.
    pub rating: PostRating,
    /// The post's description.
    pub description: String,

    /// If the post has a parent, the ID of the parent post. `None` if the post has no parent.
    pub parent_id: Option<u64>,
    /// A list of post IDs of this post's children.
    pub children: Vec<u64>,
    /// A list of the post's sources.
    pub sources: Vec<String>,

    /// If the post has any notes.
    pub has_notes: bool,
    /// If the post has any comments.
    pub has_comments: bool,

    /// The number of users who have the post in their favorites.
    pub fav_count: u64,
    /// The post's score.
    pub score: i64,

    /// Absolute URL to the filename. Unavailable for deleted posts.
    pub file_url: Option<String>,
    /// The post's extension. Unavailable for deleted posts.
    pub file_ext: Option<PostFormat>,
    /// Size (in bytes) of the post. Unavailable for deleted posts.
    pub file_size: Option<u64>,

    /// Width of the image.
    pub width: u64,
    /// Height of the image.
    pub height: u64,

    /// Absolute URL of the sample (scaled) filename. Unavailable for deleted posts.
    pub sample_url: Option<String>,
    /// Width of the sample (scaled) image. Unavailable for deleted posts.
    pub sample_width: Option<u64>,
    /// Height of the sample (scaled) image. Unavailable for deleted posts.
    pub sample_height: Option<u64>,

    /// Absolute URL of the preview (thumbnail) filename. Unavailable for deleted posts.
    pub preview_url: Option<String>,
    /// Width of the preview (thumbnail) image. Unavailable for deleted posts.
    pub preview_width: Option<u64>,
    /// Height of the preview (thumbnail) image. Unavailable for deleted posts.
    pub preview_height: Option<u64>,
}

impl Post {
    /// Returns `true` if this post is deleted. Equivalent to calling [`PostStatus::is_deleted()`]
    /// on this post's [`status`].
    ///
    /// [`PostStatus::is_deleted()`]: enum.PostStatus.html#method.is_deleted
    /// [`status`]: #structfield.status
    pub fn is_deleted(&self) -> bool {
        self.status.is_deleted()
    }
}

impl Default for Post {
    fn default() -> Post {
        Post {
            raw: Default::default(),

            id: Default::default(),
            md5: Default::default(),
            status: Default::default(),

            author: Default::default(),
            creator_id: Default::default(),
            created_at: Utc.timestamp(0, 0), // here is the bad boy

            artists: Default::default(),
            tags: Default::default(),
            rating: Default::default(),
            description: Default::default(),

            parent_id: Default::default(),
            children: Default::default(),
            sources: Default::default(),

            has_notes: Default::default(),
            has_comments: Default::default(),

            fav_count: Default::default(),
            score: Default::default(),

            file_url: Default::default(),
            file_ext: Default::default(),
            file_size: Default::default(),

            width: Default::default(),
            height: Default::default(),

            sample_url: Default::default(),
            sample_width: Default::default(),
            sample_height: Default::default(),

            preview_url: Default::default(),
            preview_width: Default::default(),
            preview_height: Default::default(),
        }
    }
}

impl TryFrom<&JsonValue> for Post {
    type Error = super::error::Error;

    fn try_from(v: &JsonValue) -> Rs621Result<Self> {
        Ok(Post {
            raw: v.to_string(),

            id: get_json_value_as(&v, "id", JsonValue::as_u64)?,
            md5: v["md5"].as_str().map(String::from),
            status: get_json_value_as(&v, "status", |v| {
                // we need to give a tuple to try_from to give it the delreason if there's any
                PostStatus::try_from((v, v["delreason"].as_str())).ok()
            })?,

            author: get_json_value_as(&v, "author", JsonValue::as_str)?.to_string(),
            creator_id: v["creator_id"].as_u64(),
            created_at: get_json_api_time(&v, "created_at")?,

            artists: get_json_value_as(&v, "artist", JsonValue::as_array)?
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),

            tags: get_json_value_as(&v, "tags", JsonValue::as_str)?
                .split_whitespace()
                .map(String::from)
                .collect(),

            rating: get_json_value_as(&v, "rating", |v| PostRating::try_from(v).ok())?,

            description: get_json_value_as(&v, "description", JsonValue::as_str)?.to_string(),

            parent_id: v["parent_id"].as_u64(),
            children: v["children"].as_str().map_or_else(Vec::new, |c| {
                if c.is_empty() {
                    Vec::new()
                } else {
                    c.split(',').map(|id| id.parse().unwrap()).collect()
                }
            }),

            sources: v["sources"].as_array().map_or_else(Vec::new, |v| {
                v.iter().map(|v| v.as_str().unwrap().to_string()).collect()
            }),

            has_notes: get_json_value_as(&v, "has_notes", JsonValue::as_bool)?,
            has_comments: get_json_value_as(&v, "has_comments", JsonValue::as_bool)?,

            fav_count: get_json_value_as(&v, "fav_count", JsonValue::as_u64)?,
            score: get_json_value_as(&v, "score", JsonValue::as_i64)?,

            file_url: match v["status"].as_str() {
                Some("deleted") => None,
                _ => Some(get_json_value_as(&v, "file_url", JsonValue::as_str)?.to_string()),
            },
            file_ext: v["file_ext"]
                .as_str()
                .map(PostFormat::try_from)
                .map(Result::unwrap),
            file_size: v["file_size"].as_u64(),

            width: get_json_value_as(&v, "width", JsonValue::as_u64)?,
            height: get_json_value_as(&v, "height", JsonValue::as_u64)?,

            sample_url: v["sample_url"].as_str().map(String::from),
            sample_width: v["sample_width"].as_u64(),
            sample_height: v["sample_height"].as_u64(),

            preview_url: match v["status"].as_str() {
                Some("deleted") => None,
                _ => Some(get_json_value_as(&v, "preview_url", JsonValue::as_str)?.to_string()),
            },
            preview_width: v["preview_width"].as_u64(),
            preview_height: v["preview_height"].as_u64(),
        })
    }
}

impl Client {
    /// Returns the post with the given ID.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let post = client.get_post(8595)?;
    ///
    /// assert_eq!(post.id, 8595);
    /// # Ok(()) }
    /// ```
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub async fn get_post(&self, id: u64) -> Rs621Result<Post> {
        let body = self
            .get_json_endpoint(&format!("/post/show.json?id={}", id))
            .await?;

        Post::try_from(&body)
    }

    /// Returns an iterator over all the posts on the website.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for post in client
    ///     .post_list()
    ///     .take(3)
    /// {
    ///     assert!(post?.id != 0);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub fn post_list<'a>(&'a self) -> PostStream<'a> {
        PostStream::new::<Query>(self, None, None)
    }

    /// Returns an iterator over all the posts matching the given tags.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::PostRating;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for post in client
    ///     .post_search(&["fluffy", "rating:s"][..])
    ///     .take(3)
    /// {
    ///     assert_eq!(post?.rating, PostRating::Safe);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub fn post_search<'a, T: Into<Query>>(&'a self, tags: T) -> PostStream<'a> {
        PostStream::new(self, Some(tags), None)
    }

    /// Returns an iterator over all the posts with an ID smaller than `before_id`.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for post in client
    ///     .post_list_before(123456)
    ///     .take(5)
    /// {
    ///     assert!(post?.id < 123456);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub fn post_list_before<'a>(&'a self, before_id: u64) -> PostStream<'a> {
        PostStream::new::<Query>(self, None, Some(before_id))
    }

    /// Returns an iterator over all the posts matching the tags and with an ID smaller than
    /// `before_id`.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::PostRating;
    /// # fn main() -> Result<(), rs621::error::Error> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    ///
    /// for post in client
    ///     .post_search_before(&["fluffy", "rating:s"][..], 123456)
    ///     .take(5)
    /// {
    ///     let post = post?;
    ///     assert!(post.id < 123456);
    ///     assert_eq!(post.rating, PostRating::Safe);
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// _Note: This function performs a request; it will be subject to a short sleep time to ensure
    /// that the API rate limit isn't exceeded._
    pub fn post_search_before<'a, T: Into<Query>>(
        &'a self,
        tags: T,
        before_id: u64,
    ) -> PostStream<'a> {
        PostStream::new(self, Some(tags), Some(before_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, Matcher};

    #[tokio::test]
    async fn search_ordered() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        const REQ_TAGS: &str = "fluffy%20rating%3As%20order%3Ascore";

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/post/index.json?limit={}&page=1&tags={}",
                ITER_CHUNK_SIZE, REQ_TAGS
            )),
        )
        .with_body(include_str!(
            "mocked/320_page-1_fluffy_rating-s_order-score.json"
        ))
        .create();

        assert_eq!(
            client
                .post_search(&["fluffy", "rating:s", "order:score"][..])
                .take(100)
                .collect::<Vec<_>>()
                .await,
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

    #[tokio::test]
    async fn search_above_limit_ordered() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        const REQ_TAGS: &str = "fluffy%20rating%3As%20order%3Ascore";
        const PAGES: [&str; 2] = [
            include_str!("mocked/320_page-1_fluffy_rating-s_order-score.json"),
            include_str!("mocked/320_page-2_fluffy_rating-s_order-score.json"),
        ];

        let _m = [
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/post/index.json?limit={}&page=1&tags={}",
                    ITER_CHUNK_SIZE, REQ_TAGS
                )),
            )
            .with_body(PAGES[0])
            .create(),
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/post/index.json?limit={}&page=2&tags={}",
                    ITER_CHUNK_SIZE, REQ_TAGS
                )),
            )
            .with_body(PAGES[1])
            .create(),
        ];

        assert_eq!(
            client
                .post_search(&["fluffy", "rating:s", "order:score"][..])
                .take(400)
                .collect::<Vec<_>>()
                .await,
            serde_json::from_str::<JsonValue>(PAGES[0])
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .chain(
                    serde_json::from_str::<JsonValue>(PAGES[1])
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

    #[tokio::test]
    async fn search_before_id() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/320_before-1869409_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .take(80)
            .map(Post::try_from)
            .collect();

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/post/index.json?limit={}&before_id=1869409&tags=fluffy%20rating%3As",
                ITER_CHUNK_SIZE
            )),
        )
        .with_body(response)
        .create();

        assert_eq!(
            client
                .post_search_before(&["fluffy", "rating:s"][..], 1869409)
                .take(80)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn search_above_limit() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/400_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(Post::try_from)
            .collect();

        let _m = [
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/post/index.json?limit={}&tags=fluffy%20rating%3As",
                    ITER_CHUNK_SIZE
                )),
            )
            .with_body(include_str!("mocked/320_fluffy_rating-s.json"))
            .create(),
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/post/index.json?limit={}&before_id=1869409&tags={}",
                    ITER_CHUNK_SIZE, "fluffy%20rating%3As"
                )),
            )
            .with_body(include_str!(
                "mocked/320_before-1869409_fluffy_rating-s.json"
            ))
            .create(),
        ];

        assert_eq!(
            client
                .post_search(&["fluffy", "rating:s"][..])
                .take(400)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn list_above_limit() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/400_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .map(Post::try_from)
            .collect();

        let _m = [
            mock(
                "GET",
                Matcher::Exact(format!("/post/index.json?limit={}", ITER_CHUNK_SIZE)),
            )
            .with_body(include_str!("mocked/320_fluffy_rating-s.json"))
            .create(),
            mock(
                "GET",
                Matcher::Exact(format!(
                    "/post/index.json?limit={}&before_id=1869409",
                    ITER_CHUNK_SIZE,
                )),
            )
            .with_body(include_str!(
                "mocked/320_before-1869409_fluffy_rating-s.json"
            ))
            .create(),
        ];

        assert_eq!(
            client.post_list().take(400).collect::<Vec<_>>().await,
            expected
        );
    }

    #[tokio::test]
    async fn search_no_result() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = "[]";
        let expected = Vec::new();

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/post/index.json?limit={}&tags=fluffy%20rating%3As",
                ITER_CHUNK_SIZE
            )),
        )
        .with_body(response)
        .create();

        assert_eq!(
            client
                .post_search(&["fluffy", "rating:s"][..])
                .take(5)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn search_simple() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/320_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .take(5)
            .map(Post::try_from)
            .collect();

        let _m = mock(
            "GET",
            Matcher::Exact(format!(
                "/post/index.json?limit={}&tags=fluffy%20rating%3As",
                ITER_CHUNK_SIZE
            )),
        )
        .with_body(response)
        .create();

        assert_eq!(
            client
                .post_search(&["fluffy", "rating:s"][..])
                .take(5)
                .collect::<Vec<_>>()
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn list_simple() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/320_fluffy_rating-s.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected: Vec<_> = response_json
            .as_array()
            .unwrap()
            .iter()
            .take(5)
            .map(Post::try_from)
            .collect();

        let _m = mock(
            "GET",
            Matcher::Exact(format!("/post/index.json?limit={}", ITER_CHUNK_SIZE)),
        )
        .with_body(response)
        .create();

        assert_eq!(
            client.post_list().take(5).collect::<Vec<_>>().await,
            expected
        );
    }

    #[tokio::test]
    async fn get_post_by_id() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let response = include_str!("mocked/id_8595.json");
        let response_json = serde_json::from_str::<JsonValue>(response).unwrap();
        let expected = Post::try_from(&response_json).unwrap();

        let _m = mock("GET", "/post/show.json?id=8595")
            .with_body(response)
            .create();

        assert_eq!(client.get_post(8595).await, Ok(expected));
    }

    #[tokio::test]
    async fn post_format_from_json() {
        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("jpg"))),
            Ok(PostFormat::JPG)
        );

        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("png"))),
            Ok(PostFormat::PNG)
        );

        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("gif"))),
            Ok(PostFormat::GIF)
        );

        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("swf"))),
            Ok(PostFormat::SWF)
        );

        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("webm"))),
            Ok(PostFormat::WEBM)
        );

        assert_eq!(
            PostFormat::try_from(&JsonValue::String(String::from("owo"))),
            Err(())
        );

        assert_eq!(PostFormat::try_from(&JsonValue::Null), Err(()));
    }

    #[tokio::test]
    async fn post_rating_from_json() {
        assert_eq!(
            PostRating::try_from(&JsonValue::String(String::from("s"))),
            Ok(PostRating::Safe)
        );

        assert_eq!(
            PostRating::try_from(&JsonValue::String(String::from("q"))),
            Ok(PostRating::Questionable)
        );

        assert_eq!(
            PostRating::try_from(&JsonValue::String(String::from("e"))),
            Ok(PostRating::Explicit)
        );

        assert_eq!(
            PostRating::try_from(&JsonValue::String(String::from("?"))),
            Err(())
        );

        assert_eq!(PostRating::try_from(&JsonValue::Null), Err(()));
    }

    #[tokio::test]
    async fn post_status_from_json() {
        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("active")), None)),
            Ok(PostStatus::Active)
        );

        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("flagged")), None)),
            Ok(PostStatus::Flagged)
        );

        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("pending")), None)),
            Ok(PostStatus::Pending)
        );

        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("deleted")), None)),
            Ok(PostStatus::Deleted(String::from("")))
        );

        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("deleted")), Some("foo"))),
            Ok(PostStatus::Deleted(String::from("foo")))
        );

        assert_eq!(
            PostStatus::try_from((&JsonValue::String(String::from("invalid")), None)),
            Err(())
        );

        assert_eq!(PostStatus::try_from((&JsonValue::Null, None)), Err(()));
    }

    #[tokio::test]
    async fn post_from_json_basic() {
        let example_json = include_str!("mocked/id_8595.json");

        let parsed = serde_json::from_str::<JsonValue>(example_json).unwrap();
        let post = Post::try_from(&parsed).unwrap();

        assert_eq!(post.raw, parsed.to_string());

        assert_eq!(post.id, 8595);
        assert_eq!(
            post.md5,
            Some(String::from("e9fbd2f2d0703a9775f245d55b9a0f9f"))
        );
        assert_eq!(post.status, PostStatus::Active);

        assert_eq!(post.author, String::from("Anomynous"));
        assert_eq!(post.creator_id, Some(46));
        assert_eq!(post.created_at, Utc.timestamp(1182709502, 993870000));

        assert_eq!(post.artists, vec![String::from("jessica_willard")]);
        assert_eq!(
            post.tags,
            vec![
                String::from("2005"),
                String::from("alley"),
                String::from("anthro"),
                String::from("ball"),
                String::from("bottomwear"),
                String::from("brown_fur"),
                String::from("canid"),
                String::from("canine"),
                String::from("canis"),
                String::from("child"),
                String::from("clothed"),
                String::from("clothing"),
                String::from("colored_pencil_(artwork)"),
                String::from("commentary"),
                String::from("cub"),
                String::from("domestic_cat"),
                String::from("domestic_dog"),
                String::from("duo"),
                String::from("english_text"),
                String::from("felid"),
                String::from("feline"),
                String::from("felis"),
                String::from("female"),
                String::from("fur"),
                String::from("ghetto"),
                String::from("grass"),
                String::from("happy"),
                String::from("jessica_willard"),
                String::from("male"),
                String::from("mammal"),
                String::from("mixed_media"),
                String::from("multicolored_fur"),
                String::from("outside"),
                String::from("pants"),
                String::from("pen_(artwork)"),
                String::from("playing"),
                String::from("politics"),
                String::from("racism"),
                String::from("shirt"),
                String::from("sign"),
                String::from("skirt"),
                String::from("smile"),
                String::from("text"),
                String::from("topwear"),
                String::from("traditional_media_(artwork)"),
                String::from("white_fur"),
                String::from("young"),
            ]
        );
        assert_eq!(post.rating, PostRating::Safe);
        assert_eq!(post.description, String::from(""));

        assert_eq!(post.parent_id, None);
        assert_eq!(post.children, vec![128898]);
        assert_eq!(
            post.sources,
            vec![
                String::from("Jessica Willard, \"jw-babysteps.jpg\""),
                String::from("http://us-p.vclart.net/vcl/Artists/J-Willard/jw-babysteps.jpg"),
                String::from("http://www.furaffinity.net/view/185399/"),
            ]
        );

        assert_eq!(post.has_notes, false);
        assert_eq!(post.has_comments, true);

        assert_eq!(post.fav_count, 159);
        assert_eq!(post.score, 76);

        assert_eq!(
            post.file_url,
            Some(String::from(
                "https://static1.e621.net/data/e9/fb/e9fbd2f2d0703a9775f245d55b9a0f9f.jpg"
            ))
        );
        assert_eq!(post.file_ext, Some(PostFormat::JPG));
        assert_eq!(post.file_size, Some(135618));

        assert_eq!(post.width, 800);
        assert_eq!(post.height, 616);

        assert_eq!(
            post.sample_url,
            Some(String::from(
                "https://static1.e621.net/data/e9/fb/e9fbd2f2d0703a9775f245d55b9a0f9f.jpg"
            ))
        );
        assert_eq!(post.sample_width, Some(800));
        assert_eq!(post.sample_height, Some(616));

        assert_eq!(
            post.preview_url,
            Some(String::from(
                "https://static1.e621.net/data/preview/e9/fb/e9fbd2f2d0703a9775f245d55b9a0f9f.jpg"
            ))
        );

        assert_eq!(post.preview_width, Some(150));
        assert_eq!(post.preview_height, Some(115));
    }

    #[tokio::test]
    async fn post_default() {
        assert_eq!(
            Post::default(),
            Post {
                raw: String::from(""),

                id: 0,
                md5: None,
                status: PostStatus::Pending,

                author: String::from(""),
                creator_id: None,
                created_at: Utc.timestamp(0, 0),

                artists: Vec::new(),
                tags: Vec::new(),
                rating: PostRating::Explicit,
                description: String::from(""),

                parent_id: None,
                children: Vec::new(),
                sources: Vec::new(),

                has_notes: false,
                has_comments: false,

                fav_count: 0,
                score: 0,

                file_url: None,
                file_ext: None,
                file_size: None,

                width: 0,
                height: 0,

                sample_url: None,
                sample_width: None,
                sample_height: None,

                preview_url: None,
                preview_width: None,
                preview_height: None,
            }
        );
    }

    #[tokio::test]
    async fn post_format_to_string() {
        assert_eq!(PostFormat::JPG.to_string(), "JPG");
        assert_eq!(PostFormat::PNG.to_string(), "PNG");
        assert_eq!(PostFormat::GIF.to_string(), "GIF");
        assert_eq!(PostFormat::SWF.to_string(), "SWF");
        assert_eq!(PostFormat::WEBM.to_string(), "WEBM");
    }

    #[tokio::test]
    async fn post_rating_to_string() {
        assert_eq!(PostRating::Safe.to_string(), "safe");
        assert_eq!(PostRating::Questionable.to_string(), "questionable");
        assert_eq!(PostRating::Explicit.to_string(), "explicit");
    }

    #[tokio::test]
    async fn post_status_is_deleted() {
        assert!(PostStatus::Deleted(String::from("foo")).is_deleted());
    }

    #[tokio::test]
    async fn post_status_is_not_deleted() {
        assert!(!PostStatus::Active.is_deleted());
    }

    #[tokio::test]
    async fn post_is_deleted() {
        let post = Post {
            status: PostStatus::Deleted(String::from("foo")),
            ..Default::default()
        };

        assert!(post.is_deleted());
    }

    #[tokio::test]
    async fn post_is_not_deleted() {
        let post = Post {
            status: PostStatus::Active,
            ..Default::default()
        };

        assert!(!post.is_deleted());
    }
}
