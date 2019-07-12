use chrono::{offset::Utc, DateTime, TimeZone};
use serde_json;
use std::fmt;

/// Post status.
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

/// Post's rating.
pub enum PostRating {
    Safe,
    Questionnable,
    Explicit,
}

impl fmt::Display for PostRating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PostRating::Explicit => write!(f, "Explicit"),
            PostRating::Questionnable => write!(f, "Questionable"),
            PostRating::Safe => write!(f, "Safe"),
        }
    }
}

/// Post file formats/extensions.
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
pub struct Post {
    /// The raw JSON description of the post (from the API).
    pub raw: String,

    /// The ID of the post.
    pub id: u64,
    /// The post's MD5 hash
    pub md5: Option<String>,
    /// The status of the post.
    pub status: PostStatus,

    /// Username of the user who uploaded the post.
    pub author: String,
    /// User ID of the user who uploaded the post.
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

    /// If the post has a parent, the ID of the parent post.
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

    /// Absolute URL to the filename.
    pub file_url: String,
    /// The post's extension.
    pub file_ext: Option<PostFormat>,
    /// Size (in bytes) of the post.
    pub file_size: Option<u64>,

    /// Width of the image.
    pub width: u64,
    /// Height of the image.
    pub height: u64,

    /// Absolute URL of the sample (scaled) filename.
    pub sample_url: Option<String>,
    /// Width of the sample (scaled) image.
    pub sample_width: Option<u64>,
    /// Height of the sample (scaled) image.
    pub sample_height: Option<u64>,

    /// Absolute URL of the preview (thumbnail) filename.
    pub preview_url: String,
    /// Width of the preview (thumbnail) image.
    pub preview_width: Option<u64>,
    /// Height of the preview (thumbnail) image.
    pub preview_height: Option<u64>,
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

impl From<&serde_json::Value> for Post {
    fn from(v: &serde_json::Value) -> Self {
        Post {
            raw: v.to_string(),

            id: v["id"].as_u64().unwrap(),
            md5: v["md5"].as_str().map(String::from),
            status: match v["status"].as_str() {
                Some("active") => PostStatus::Active,
                Some("flagged") => PostStatus::Flagged,
                Some("pending") => PostStatus::Pending,
                Some("deleted") => {
                    PostStatus::Deleted(v["delreason"].as_str().unwrap().to_string())
                }
                _ => unreachable!(),
            },

            author: v["author"].as_str().unwrap().to_string(),
            creator_id: v["creator_id"].as_u64(),
            created_at: Utc.timestamp(
                v["created_at"]["s"].as_i64().unwrap(),
                v["created_at"]["n"].as_u64().unwrap() as u32,
            ),

            artists: v["artist"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),

            tags: v["tags"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .map(String::from)
                .collect(),

            rating: match v["rating"].as_str().unwrap() {
                "e" => PostRating::Explicit,
                "q" => PostRating::Questionnable,
                "s" => PostRating::Safe,
                _ => unreachable!(),
            },

            description: v["description"].as_str().unwrap().to_string(),

            parent_id: v["parent_id"].as_u64(),
            children: v["children"].as_str().map_or_else(Vec::new, |c| {
                if c.is_empty() {
                    Vec::new()
                } else {
                    c.split(',').map(|id| id.parse().unwrap()).collect()
                }
            }),

            sources: v["children"].as_array().map_or_else(Vec::new, |v| {
                v.iter().map(|v| v.as_str().unwrap().to_string()).collect()
            }),

            has_notes: v["has_notes"].as_bool().unwrap(),
            has_comments: v["has_comments"].as_bool().unwrap(),

            fav_count: v["fav_count"].as_u64().unwrap(),
            score: v["score"].as_i64().unwrap(),

            file_url: v["file_url"].as_str().unwrap().to_string(),
            file_ext: v["file_ext"].as_str().map(|v| match v {
                "jpg" => PostFormat::JPG,
                "png" => PostFormat::PNG,
                "gif" => PostFormat::GIF,
                "swf" => PostFormat::SWF,
                "webm" => PostFormat::WEBM,
                _ => unreachable!(),
            }),
            file_size: v["file_size"].as_u64(),

            width: v["width"].as_u64().unwrap(),
            height: v["height"].as_u64().unwrap(),

            sample_url: v["sample_url"].as_str().map(String::from),
            sample_width: v["sample_width"].as_u64(),
            sample_height: v["sample_height"].as_u64(),

            preview_url: v["preview_url"].as_str().unwrap().to_string(),
            preview_width: v["preview_width"].as_u64(),
            preview_height: v["preview_height"].as_u64(),
        }
    }
}
