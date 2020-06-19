use {
    super::{client::Client, error::Result as Rs621Result},
    chrono::{offset::Utc, DateTime},
    derivative::Derivative,
    futures::{
        prelude::*,
        task::{Context, Poll},
    },
    serde::{
        de::{self, MapAccess, Visitor},
        Deserialize, Deserializer,
    },
    std::pin::Pin,
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
    pub url: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostPreview {
    pub width: u64,
    pub height: u64,
    pub url: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct PostSample {
    pub width: u64,
    pub height: u64,
    pub url: String,
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
    #[serde(deserialize_with = "PostSample::from_json")]
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

impl PostSample {
    fn from_json<'de, D>(de: D) -> Result<Option<PostSample>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Has,
            Width,
            Height,
            Url,
        }

        struct PostSampleVisitor;

        impl<'de> Visitor<'de> for PostSampleVisitor {
            type Value = Option<PostSample>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct PostSample")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Option<PostSample>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut has = None;
                let mut width = None;
                let mut height = None;
                let mut url = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Has => {
                            if has.is_some() {
                                return Err(de::Error::duplicate_field("has"));
                            }

                            has = Some(map.next_value()?);
                        }
                        Field::Width => {
                            if width.is_some() {
                                return Err(de::Error::duplicate_field("width"));
                            }

                            width = Some(map.next_value()?);
                        }
                        Field::Height => {
                            if height.is_some() {
                                return Err(de::Error::duplicate_field("height"));
                            }

                            height = Some(map.next_value()?);
                        }
                        Field::Url => {
                            if url.is_some() {
                                return Err(de::Error::duplicate_field("url"));
                            }

                            url = Some(map.next_value()?);
                        }
                    }
                }

                let has = has.ok_or_else(|| de::Error::missing_field("has"))?;
                let width = width.ok_or_else(|| de::Error::missing_field("width"))?;
                let height = height.ok_or_else(|| de::Error::missing_field("height"))?;
                let url = url.ok_or_else(|| de::Error::missing_field("url"))?;

                if let Some(true) = has {
                    Ok(None)
                } else {
                    Ok(Some(PostSample { width, height, url }))
                }
            }
        }

        const FIELDS: &'static [&'static str] = &["has", "width", "height", "url"];
        de.deserialize_struct("PostSample", FIELDS, PostSampleVisitor)
    }
}

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
                            Ok(mut body) => {
                                // put everything in the chunk
                                this.chunk =
                                    match serde_json::from_value::<Vec<Post>>(body["posts"].take())
                                    {
                                        Ok(vec) => {
                                            vec.into_iter().rev().map(|post| Ok(post)).collect()
                                        }
                                        Err(e) => vec![Err(e.into())],
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
                        "/posts.json?limit={}{}{}",
                        ITER_CHUNK_SIZE,
                        if let Some(Query { ordered: true, .. }) = this.query {
                            this.page += 1;
                            format!("&page={}", this.page)
                        } else {
                            match this.last_id {
                                Some(i) => format!("&page=b{}", i),
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

        Ok(serde_json::from_value(body)?)
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
