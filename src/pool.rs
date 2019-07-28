use super::client::Client;
use super::post::Post;
use chrono::{offset::Utc, DateTime};

/// Represents results returned by `pool/index.json`.
#[derive(Debug)]
pub struct PoolListResult<'a, C: reqwest_mock::Client> {
    client: &'a Client<C>,
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

#[derive(Debug, PartialEq)]
pub struct Pool {
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
