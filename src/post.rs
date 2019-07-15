use chrono::{offset::Utc, DateTime, TimeZone};
use serde_json;
use serde_json::Value as JsonValue;
use std::convert::TryFrom;
use std::fmt;

/// Post status.
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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

fn get_json_value_as<'a, T, F>(v: &'a JsonValue, k: &str, p: F) -> Result<T, super::error::Error>
where
    F: FnOnce(&'a JsonValue) -> Option<T>,
{
    let value = &v[k];
    p(&value).ok_or(super::error::Error::PostDeserialization(
        k.to_string(),
        v.to_string(),
    ))
}

impl TryFrom<&JsonValue> for Post {
    type Error = super::error::Error;

    fn try_from(v: &JsonValue) -> super::error::Result<Self> {
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
            created_at: Utc.timestamp(
                get_json_value_as(&v["created_at"], "s", JsonValue::as_i64)?,
                get_json_value_as(&v["created_at"], "n", JsonValue::as_u64)? as u32,
            ),

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

#[cfg(test)]
mod tests {
    use super::*;

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
        let example_json = include_str!("mocked/show_id_8595.json");

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
