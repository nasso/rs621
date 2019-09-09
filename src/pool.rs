use super::{
    client::Client,
    error::{Error, Result as Rs621Result},
    post::Post,
    utils::{get_json_api_time, get_json_value_as},
};
use chrono::{offset::Utc, DateTime};
use serde_json::Value as JsonValue;
use std::convert::TryFrom;

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

// Conversions
impl<C: reqwest_mock::Client> TryFrom<PoolListResult<'_, C>> for Pool {
    type Error = Error;

    fn try_from(_r: PoolListResult<'_, C>) -> Rs621Result<Pool> {
        unimplemented!()
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
            .stub(Url::parse(&format!("https://e621.net/pool/show.json?id={}", 18274)).unwrap())
            .method(Method::GET)
            .response()
            .body(include_str!("mocked/pool_18274.json"))
            .mock()
            .is_ok());

        let pool = client.get_pool(18274).unwrap();
        assert_eq!(pool.id, 18274);
    }
}
