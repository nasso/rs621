use super::{
    client::{Client, Query, ITER_CHUNK_SIZE},
    error::Result as Rs621Result,
    utils::{get_json_api_time, get_json_value_as},
};
use chrono::{offset::Utc, DateTime, TimeZone};
use serde_json::Value as JsonValue;
use std::{convert::TryFrom, fmt};

/// Iterator returning posts from a search query.
#[derive(Debug)]
pub struct PostIter<'a, C: reqwest_mock::Client> {
    client: &'a Client<C>,
    query: Query,

    last_id: Option<u64>,
    page: u64,
    chunk: Vec<Rs621Result<Post>>,
    ended: bool,
}

impl<'a, C: reqwest_mock::Client> PostIter<'a, C> {
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
    type Item = Rs621Result<Post>;

    fn next(&mut self) -> Option<Rs621Result<Post>> {
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

impl fmt::Display for Post {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let PostStatus::Deleted(ref reason) = &self.status {
            writeln!(f, "#{} (deleted: {})", self.id, reason)?;
        } else {
            write!(f, "#{} by ", self.id)?;

            let artist_count = self.artists.len();
            for i in 0..artist_count {
                match artist_count - i {
                    1 => writeln!(f, "{}", self.artists[i])?,
                    2 => write!(f, "{} and ", self.artists[i])?,
                    _ => write!(f, "{}, ", self.artists[i])?,
                }
            }
        }

        writeln!(f, "Rating: {}", self.rating)?;

        writeln!(f, "Score: {}", self.score)?;
        writeln!(f, "Favs: {}", self.fav_count)?;

        if let Some(ref t) = self.file_ext {
            writeln!(f, "Type: {}", t)?;
        }

        writeln!(f, "Created at: {}", self.created_at)?;
        writeln!(f, "Tags: {}", self.tags.join(", "))?;
        write!(f, "Description: {}", self.description)?;

        Ok(())
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

impl<C: reqwest_mock::Client> Client<C> {
    /// Returns the post with the given ID.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::post::Post;
    /// # fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("MyProject/1.0 (by username on e621)")?;
    /// let post = client.get_post(8595)?;
    ///
    /// assert_eq!(post.id, 8595);
    /// # Ok(()) }
    /// ```
    ///
    /// This function performs a request; it will be subject to a short sleep time to ensure that
    /// the API rate limit isn't exceeded.
    pub fn get_post(&self, id: u64) -> Rs621Result<Post> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest_mock::{Method, Url};

    #[test]
    fn list_ordered() {
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
        let mut client = Client::new_mocked(b"rs621/unit_test").unwrap();

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
    fn post_format_from_json() {
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

    #[test]
    fn post_rating_from_json() {
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

    #[test]
    fn post_status_from_json() {
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

    #[test]
    fn post_from_json_basic() {
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

    #[test]
    fn post_default() {
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

    #[test]
    fn post_format_to_string() {
        assert_eq!(PostFormat::JPG.to_string(), "JPG");
        assert_eq!(PostFormat::PNG.to_string(), "PNG");
        assert_eq!(PostFormat::GIF.to_string(), "GIF");
        assert_eq!(PostFormat::SWF.to_string(), "SWF");
        assert_eq!(PostFormat::WEBM.to_string(), "WEBM");
    }

    #[test]
    fn post_rating_to_string() {
        assert_eq!(PostRating::Safe.to_string(), "safe");
        assert_eq!(PostRating::Questionable.to_string(), "questionable");
        assert_eq!(PostRating::Explicit.to_string(), "explicit");
    }

    #[test]
    fn post_status_is_deleted() {
        assert!(PostStatus::Deleted(String::from("foo")).is_deleted());
    }

    #[test]
    fn post_status_is_not_deleted() {
        assert!(!PostStatus::Active.is_deleted());
    }

    #[test]
    fn post_is_deleted() {
        let post = Post {
            status: PostStatus::Deleted(String::from("foo")),
            ..Default::default()
        };

        assert!(post.is_deleted());
    }

    #[test]
    fn post_is_not_deleted() {
        let post = Post {
            status: PostStatus::Active,
            ..Default::default()
        };

        assert!(!post.is_deleted());
    }
}
