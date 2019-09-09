use super::error::Result as Rs621Result;
use chrono::{
    offset::{TimeZone, Utc},
    DateTime,
};
use serde_json::Value as JsonValue;

pub fn get_json_value_as<'a, T, F>(v: &'a JsonValue, k: &str, p: F) -> Rs621Result<T>
where
    F: FnOnce(&'a JsonValue) -> Option<T>,
{
    let value = &v[k];
    p(&value).ok_or(super::error::Error::Deserialization {
        key: k.to_string(),
        value: v.to_string(),
    })
}

pub fn get_json_api_time<'a>(v: &'a JsonValue, k: &str) -> Rs621Result<DateTime<Utc>> {
    Ok(Utc.timestamp(
        get_json_value_as(&v[k], "s", JsonValue::as_i64)?,
        get_json_value_as(&v[k], "n", JsonValue::as_u64)? as u32,
    ))
}
