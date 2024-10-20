use chrono::{DateTime, Utc};

use crate::client::{Client, Cursor};
use crate::error::Result as Rs621Result;

use either::Either;

use futures::stream::unfold;
use futures::{Stream, StreamExt};

use itertools::Itertools;

use serde::{Deserialize, Serialize};

use serde_repr::{Deserialize_repr, Serialize_repr};

use serde_with::formats::CommaSeparator;
use serde_with::serde_as;

use std::{fmt, num::ParseIntError, ops::Not, str::FromStr};

use thiserror::Error;

/// Kind of property a [`Tag`] describes.
#[derive(Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr, Clone, Copy)]
#[repr(u8)]
pub enum Category {
    General = 0,
    Artist = 1,
    Copyright = 3,
    Character = 4,
    Species = 5,
    Invalid = 6,
    Meta = 7,
    Lore = 8,
}

impl FromStr for Category {
    type Err = ParseCategoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let num: u8 = s.parse()?;
        let result = match num {
            0 => Self::General,
            1 => Self::Artist,
            3 => Self::Copyright,
            4 => Self::Character,
            5 => Self::Species,
            6 => Self::Invalid,
            7 => Self::Meta,
            8 => Self::Lore,
            _ => return Err(ParseCategoryError::Unknown(num)),
        };

        Ok(result)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

#[derive(Error, Debug)]
pub enum ParseCategoryError {
    #[error("category string is not a u8")]
    ParseInt(#[from] ParseIntError),

    #[error("unknown category {0}")]
    Unknown(u8),
}

/// How to sort results of a [`Query`].
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// By [`id`][Tag::id], smallest first.
    IdAsc,

    /// By [`id`][Tag::id], largest first.
    IdDsc,

    /// By [`name`][Tag::name], in alphabetical order.
    Name,

    /// By [`id`][Tag::id], largest first. See [`app/models/tag.rb`][0].
    ///
    /// [0]: https://github.com/e621ng/e621ng/blob/7866c700faa690194352433931b4ee1db063e632/app/models/tag.rb#L365-L366
    Date,

    /// By [`post_count`][Tag::post_count], largest first.
    Count,

    /// By similarity to query, when doing a fuzzy search, with most similar first.
    Similarity,
}

/// Tags are keywords used to describe a [`Post`][crate::post::Post].
#[derive(Debug, PartialEq, Eq, Deserialize, Clone)]
#[non_exhaustive]
pub struct Tag {
    pub id: u64,
    pub name: String,
    pub post_count: u64,
    pub related_tags: String, // Swagger docs say `array<string>`?
    #[serde(default)]
    pub related_tags_updated_at: Option<DateTime<Utc>>,
    pub category: Category,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

type CommaSeparated<T> = serde_with::StringWithSeparator<CommaSeparator, T>;

/// A search query returning [`Tag`] instances.
///
/// ```
/// # use rs621::tag::{Query, Order, Category};
/// # use rs621::client::Cursor;
/// let query = Query::new()
///     .per_page(1)
///     .page(Cursor::Page(2))
///     .id(3)
///     .order(Order::Similarity)
///     .fuzzy_name_matches("fuzzy")
///     .name_matches(String::from("name_matches"))
///     .name("name")
///     .names(["hello"])
///     .category(Category::Species)
///     .categories([Category::Lore, Category::Meta])
///     .hide_empty(true)
///     .has_wiki(true)
///     .has_artist(true);
/// ```
#[serde_as]
#[derive(Default, Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct Query {
    // Renaming `limit` to `per_page` because `limit` implies a maximum on the total number of
    // items returned, which isn't the case because of the auto-pagination code.
    #[serde(default, rename = "limit", skip_serializing_if = "Option::is_none")]
    per_page: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    page: Option<Cursor>,

    #[serde(
        rename = "search[id]",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    id: Option<u64>,

    #[serde(
        rename = "search[order]",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    order: Option<Order>,

    #[serde(
        rename = "search[fuzzy_name_matches]",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    fuzzy_name_matches: Option<String>,

    #[serde(
        rename = "search[name_matches]",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    name_matches: Option<String>,

    #[serde_as(as = "CommaSeparated<String>")]
    #[serde(
        rename = "search[name]",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    name: Vec<String>,

    #[serde_as(as = "CommaSeparated<Category>")]
    #[serde(
        rename = "search[category]",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    category: Vec<Category>,

    #[serde(
        rename = "search[hide_empty]",
        default,
        skip_serializing_if = "<&bool>::not"
    )]
    hide_empty: bool,

    #[serde(
        rename = "search[has_wiki]",
        default,
        skip_serializing_if = "<&bool>::not"
    )]
    has_wiki: bool,

    #[serde(
        rename = "search[has_artist]",
        default,
        skip_serializing_if = "<&bool>::not"
    )]
    has_artist: bool,
}

impl Query {
    /// Create a new instance of `Query` with the default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of items to retrieve at a time. Equivalent to the `limit` query parameter.
    pub fn per_page<T: Into<Option<u16>>>(mut self, per_page: T) -> Self {
        self.per_page = per_page.into();
        self
    }

    /// Set the page/offset to start retrieving tags from.
    ///
    /// **Note:** Setting either [`Cursor::Before`] or [`Cursor::After`] will clear any previously
    /// set ordering ([`Self::order`].)
    pub fn page<T: Into<Option<Cursor>>>(mut self, page: T) -> Self {
        self.page = page.into();
        if !matches!(self.page, None | Some(Cursor::Page(_))) {
            self.order = None;
        }
        self
    }

    /// Search by [`Tag::id`].
    pub fn id<T: Into<Option<u64>>>(mut self, id: T) -> Self {
        self.id = id.into();
        self
    }

    /// Set the sort order for returned tags.
    ///
    /// **Note:** Setting this to a non-`None` value will clear [`Self::page`], unless
    /// [`Self::page`] is set to [`Cursor::Page`].
    pub fn order<T: Into<Option<Order>>>(mut self, order: T) -> Self {
        self.order = order.into();
        if self.order.is_some() && !matches!(self.page, None | Some(Cursor::Page(_))) {
            self.page = None;
        }
        self
    }

    /// Search by [`Tag::name`], finding results that are similar but not necessarily an exact
    /// match. Useful in combination with [`Order::Similarity`].
    pub fn fuzzy_name_matches<T: Into<Option<S>>, S: Into<String>>(mut self, name: T) -> Self {
        self.fuzzy_name_matches = name.into().map(Into::into);
        self
    }

    /// Search by [`Tag::name`], exactly. Supports wildcards using `*`.
    pub fn name_matches<T: Into<Option<S>>, S: Into<String>>(mut self, name: T) -> Self {
        self.name_matches = name.into().map(Into::into);
        self
    }

    /// Search by [`Tag::name`], exactly.
    ///
    /// This is a convenience function for setting a single value. For multiple values, see
    /// [`Self::names`].
    pub fn name<T: Into<Option<S>>, S: Into<String>>(mut self, name: T) -> Self {
        self.name = name.into().into_iter().map(Into::into).collect();
        self
    }

    /// Search by [`Tag::name`], exactly.
    ///
    /// For a convenience function to set a single value, see [`Self::name`].
    pub fn names<I: IntoIterator<Item = S>, S: Into<String>>(mut self, names: I) -> Self {
        self.name = names.into_iter().map(Into::into).collect();
        self
    }

    /// Search by [`Tag::category`].
    ///
    /// This is a convenience function for setting a single value. For multiple values, see
    /// [`Self::categories`].
    pub fn category<T: Into<Option<Category>>>(mut self, category: T) -> Self {
        self.category = category.into().into_iter().collect();
        self
    }

    /// Search by [`Tag::category`], exactly.
    ///
    /// For a convenience function to set a single value, see [`Self::category`].
    pub fn categories<I: IntoIterator<Item = Category>>(mut self, categories: I) -> Self {
        self.category = categories.into_iter().collect();
        self
    }

    /// Whether to hide results that have no posts.
    pub fn hide_empty(mut self, hide_empty: bool) -> Self {
        self.hide_empty = hide_empty;
        self
    }

    /// Whether to only show results that have an associated wiki page.
    pub fn has_wiki(mut self, has_wiki: bool) -> Self {
        self.has_wiki = has_wiki;
        self
    }

    /// Whether to only show results that either are artists or have a matching artist tag (eg.
    /// `lips` and `lips_(artist)`).
    pub fn has_artist(mut self, has_artist: bool) -> Self {
        self.has_artist = has_artist;
        self
    }
}

impl Client {
    /// Returns a Stream over all the [`Tag`]s matching the search query.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::tag::Query;
    /// use futures::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    ///
    /// let query = Query::new()
    ///     .name("fluffy")
    ///     .per_page(1);
    ///
    /// let tag_stream = client.tag_search(query);
    /// futures::pin_mut!(tag_stream);
    ///
    /// while let Some(tag) = tag_stream.next().await {
    ///     assert_eq!(tag?.name, "fluffy");
    /// }
    /// # Ok(()) }
    /// ```
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    pub fn tag_search(
        &self,
        query: Query,
    ) -> impl Stream<Item = Rs621Result<Tag>> + '_ + Send + Sync {
        // TODO: There should be a way to use `try_unfold` here instead.
        unfold(Some(query), move |query| self.tag_search_page(query))
            .map(futures::stream::iter)
            .flatten()
    }

    /// Returns a Stream over all the [`Tag`]s matching the search query.
    ///
    /// ```no_run
    /// # use rs621::client::Client;
    /// # use rs621::tag::Query;
    /// use futures::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> rs621::error::Result<()> {
    /// let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
    ///
    /// let query = Query::new()
    ///     .name("fluffy")
    ///     .per_page(1);
    ///
    /// let tag_stream = client.tag_search(query);
    /// futures::pin_mut!(tag_stream);
    ///
    /// while let Some(tag) = tag_stream.next().await {
    ///     assert_eq!(tag?.name, "fluffy");
    /// }
    /// # Ok(()) }
    /// ```
    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn tag_search(&self, query: Query) -> impl Stream<Item = Rs621Result<Tag>> + '_ {
        // TODO: There should be a way to use `try_unfold` here instead.
        unfold(Some(query), move |query| self.tag_search_page(query))
            .map(futures::stream::iter)
            .flatten()
    }

    async fn tag_search_page(
        &self,
        query: Option<Query>,
    ) -> Option<(impl Iterator<Item = Rs621Result<Tag>>, Option<Query>)> {
        // `/tags.json` is weird and returns `{ tags: [] }` instead of the empty array when there
        // are no results.
        #[derive(Deserialize)]
        #[serde(untagged, deny_unknown_fields)]
        enum MaybeTags {
            #[allow(dead_code)]
            Empty {
                tags: [(); 0],
            },
            Items(Vec<Tag>),
        }

        // `query` will be `None` if the previous `tag_search_page` errored. If that is the case,
        // this run of `tag_search_page` will return `None` to end the stream.
        let mut query = query?;

        let tags = match self.get_json_endpoint_query("/tags.json", &query).await {
            Err(e) => return Some((Either::Left(std::iter::once(Err(e))), None)),
            Ok(MaybeTags::Empty { .. }) => return None,
            Ok(MaybeTags::Items(i)) if i.is_empty() => return None,
            Ok(MaybeTags::Items(i)) => i,
        };

        let (min, max) = tags.iter().map(|x| x.id).minmax().into_option().unwrap();

        let next_page = match query.page {
            Some(Cursor::Before(_)) => Cursor::Before(min),
            Some(Cursor::After(_)) => Cursor::After(max),
            Some(Cursor::Page(p)) => Cursor::Page(p + 1),
            None => match query.order {
                None | Some(Order::IdDsc | Order::Date) => Cursor::Before(min),
                Some(Order::IdAsc) => Cursor::After(max),
                _ => Cursor::Page(2),
            },
        };
        query.page = Some(next_page);

        let tag_results = tags.into_iter().map(Ok);

        Some((Either::Right(tag_results), Some(query)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query() {
        let query = Query::new()
            .per_page(1)
            .page(Cursor::Page(2))
            .id(3)
            .order(Order::Similarity)
            .fuzzy_name_matches("fuzzy")
            .name_matches(String::from("name_matches"))
            .name("name")
            .names(["hello"])
            .category(Category::Species)
            .categories([Category::Lore, Category::Meta])
            .hide_empty(true)
            .has_wiki(true)
            .has_artist(true);

        assert_eq!(
            query,
            Query {
                per_page: Some(1),
                page: Some(Cursor::Page(2)),
                id: Some(3),
                order: Some(Order::Similarity),
                fuzzy_name_matches: Some("fuzzy".into()),
                name_matches: Some("name_matches".into()),
                name: vec!["hello".into()],
                category: vec![Category::Lore, Category::Meta],
                hide_empty: true,
                has_wiki: true,
                has_artist: true,
            }
        );
    }
}
