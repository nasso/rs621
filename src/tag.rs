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
    #[cfg(not(target_family = "wasm"))]
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
    #[cfg(target_family = "wasm")]
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
    use mockito::mock;

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

    #[tokio::test]
    async fn tags_paginated_ordered_by_count() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::new().order(Order::Count).per_page(1);

        let _page1 = mock("GET", "/tags.json?limit=1&search%5Border%5D=count")
            .with_body(include_str!("mocked/tags_order-count_limit-1_page-1.json"))
            .create();
        let _page2 = mock("GET", "/tags.json?limit=1&page=2&search%5Border%5D=count")
            .with_body(include_str!("mocked/tags_order-count_limit-1_page-2.json"))
            .create();
        let _page3 = mock("GET", "/tags.json?limit=1&page=3&search%5Border%5D=count")
            .with_body(include_str!("mocked/tags_empty.json"))
            .create();

        let expected = vec![
            Ok(Tag {
                id: 12054,
                name: "mammal".into(),
                post_count: 3350829,
                related_tags: concat!(
                    "mammal 300 anthro 224 hi_res 212 female 185 male 162 solo 147 ",
                    "gentle_taps 146 fur 137 hair 130 balloons 125 clothing 122 duo 111 ",
                    "canid 109 canine 107 peanut 101 skittles 99 possibly_druids 98 ",
                    "digital_media_(artwork) 89 simple_background 87 dude 84 tail 77 ",
                    "text 77 absurd_res 73 genuine_druids 72 blush 70"
                )
                .into(),
                related_tags_updated_at: "2024-10-12T17:22:13.554-04:00".parse().ok(),
                category: Category::Species,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2024-10-12T17:22:13.554-04:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 7115,
                name: "anthro".into(),
                post_count: 3288365,
                related_tags: concat!(
                    "anthro 300 mammal 236 hi_res 211 male 189 solo 165 female 162 ",
                    "clothing 159 gentle_taps 152 balloons 130 fur 118 hair 114 peanut 112 ",
                    "duo 107 possibly_druids 105 skittles 104 tail 100 simple_background 98 ",
                    "clothed 91 dude 90 canid 89 canine 88 digital_media_(artwork) 87 ",
                    "genuine_druids 82 balls 79 mutt 77"
                )
                .into(),
                related_tags_updated_at: "2024-10-11T10:06:56.303-04:00".parse().ok(),
                category: Category::General,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2024-10-11T10:06:56.303-04:00".parse().unwrap(),
            }),
        ];

        let actual = client.tag_search(query).collect::<Vec<_>>().await;

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn tags_paginated_before() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::new().per_page(2).page(Cursor::Before(12054));

        let _page1 = mock("GET", "/tags.json?limit=2&page=b12054")
            .with_body(include_str!("mocked/tags_limit-2_page-b12054.json"))
            .create();
        let _page2 = mock("GET", "/tags.json?limit=2&page=b12052")
            .with_body(include_str!("mocked/tags_limit-2_page-b12052.json"))
            .create();
        let _page3 = mock("GET", "/tags.json?limit=2&page=b12048")
            .with_body(include_str!("mocked/tags_empty.json"))
            .create();

        let expected = vec![
            Ok(Tag {
                id: 12053,
                name: "sefeiren".into(),
                post_count: 1581,
                related_tags: concat!(
                    "conditional_dnp 300 sefeiren 300 female 222 male 211 claws 176 ",
                    "anthro 173 gentle_taps 165 mammal 165 paws 161 hindpaw 151 hi_res 150 ",
                    "feral 149 fur 142 tongue 142 dude 131 duo 122 open_mouth 122 ",
                    "scalie 117 possibly_druids 116 frisky_ferals 113 horn 112 solo 112 ",
                    "peanut 108 text 104 blush 101",
                )
                .into(),
                related_tags_updated_at: "2022-04-06T06:21:26.712-04:00".parse().ok(),
                category: Category::Artist,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2022-04-06T06:21:26.713-04:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12052,
                name: "skee".into(),
                post_count: 2,
                related_tags: concat!(
                    "male 2 solo 2 skee 2 simple_background 2 shorts 1 bulge 1 ",
                    "clothing 1 genuine_outline 1 gentle_taps 1 hi_res 1 huge_balls 1 ",
                    "huge_peanut 1 hyper 1 hyper_balls 1 hyper_melancholia 1 ",
                    "hyper_peanut 1 limitedvision 1 lizard 1 anthro 1 multi_melancholia 1 ",
                    "multi_peanut 1 peanut 1 peanut_outline 1 reptile 1 scalie 1",
                )
                .into(),
                related_tags_updated_at: "2020-02-24T21:51:39.390-05:00".parse().ok(),
                category: Category::Artist,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12051,
                name: "kash".into(),
                post_count: 5,
                related_tags: concat!(
                    "lagomorph 5 mammal 5 kash 5 male 5 leporid 4 rabbit 4 anthro 4 ",
                    "solo 4 balls 3 long_ears 3 dude 3 peanut 3 hair 3 purple_hair 2 ",
                    "affection 2 blue_countershading 2 gentle_taps 2 fur 2 mutt 2 biped 2 ",
                    "backpack 2 blue_markings 2 countershading 2 markings 2 challen 2",
                )
                .into(),
                related_tags_updated_at: "2020-03-04T21:55:37.812-05:00".parse().ok(),
                category: Category::Character,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12048,
                name: "fish_bowl".into(),
                post_count: 147,
                related_tags: concat!(
                    "aquarium 108 fish_bowl 108 vivarium 108 mammal 76 marine 71 ",
                    "fish 69 hi_res 56 clothing 50 feral 49 male 49 female 48 ",
                    "water 48 fur 47 text 44 hair 42 ambiguous_gender 40 anthro 40 ",
                    "group 38 english_text 36 smile 36 simple_background 35 ",
                    "felid 34 open_mouth 34 solo 34 clothed 32",
                )
                .into(),
                related_tags_updated_at: "2021-12-11T15:59:15.118-05:00".parse().ok(),
                category: Category::General,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2021-12-11T15:59:15.118-05:00".parse().unwrap(),
            }),
        ];

        let actual = client.tag_search(query).collect::<Vec<_>>().await;

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn tags_paginated_after() {
        let client = Client::new(&mockito::server_url(), b"rs621/unit_test").unwrap();

        let query = Query::new().per_page(2).page(Cursor::After(12047));

        let _page1 = mock("GET", "/tags.json?limit=2&page=a12047")
            .with_body(include_str!("mocked/tags_limit-2_page-a12047.json"))
            .create();
        let _page2 = mock("GET", "/tags.json?limit=2&page=a12051")
            .with_body(include_str!("mocked/tags_limit-2_page-a12051.json"))
            .create();
        let _page3 = mock("GET", "/tags.json?limit=2&page=a12053")
            .with_body(include_str!("mocked/tags_empty.json"))
            .create();

        // Even with `page=a...`, each page is returned in descending order.
        let expected = vec![
            Ok(Tag {
                id: 12051,
                name: "kash".into(),
                post_count: 5,
                related_tags: concat!(
                    "lagomorph 5 mammal 5 kash 5 male 5 leporid 4 rabbit 4 anthro 4 ",
                    "solo 4 balls 3 long_ears 3 dude 3 peanut 3 hair 3 purple_hair 2 ",
                    "affection 2 blue_countershading 2 gentle_taps 2 fur 2 mutt 2 biped 2 ",
                    "backpack 2 blue_markings 2 countershading 2 markings 2 challen 2",
                )
                .into(),
                related_tags_updated_at: "2020-03-04T21:55:37.812-05:00".parse().ok(),
                category: Category::Character,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12048,
                name: "fish_bowl".into(),
                post_count: 147,
                related_tags: concat!(
                    "aquarium 108 fish_bowl 108 vivarium 108 mammal 76 marine 71 ",
                    "fish 69 hi_res 56 clothing 50 feral 49 male 49 female 48 ",
                    "water 48 fur 47 text 44 hair 42 ambiguous_gender 40 anthro 40 ",
                    "group 38 english_text 36 smile 36 simple_background 35 ",
                    "felid 34 open_mouth 34 solo 34 clothed 32",
                )
                .into(),
                related_tags_updated_at: "2021-12-11T15:59:15.118-05:00".parse().ok(),
                category: Category::General,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2021-12-11T15:59:15.118-05:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12053,
                name: "sefeiren".into(),
                post_count: 1581,
                related_tags: concat!(
                    "conditional_dnp 300 sefeiren 300 female 222 male 211 claws 176 ",
                    "anthro 173 gentle_taps 165 mammal 165 paws 161 hindpaw 151 hi_res 150 ",
                    "feral 149 fur 142 tongue 142 dude 131 duo 122 open_mouth 122 ",
                    "scalie 117 possibly_druids 116 frisky_ferals 113 horn 112 solo 112 ",
                    "peanut 108 text 104 blush 101",
                )
                .into(),
                related_tags_updated_at: "2022-04-06T06:21:26.712-04:00".parse().ok(),
                category: Category::Artist,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2022-04-06T06:21:26.713-04:00".parse().unwrap(),
            }),
            Ok(Tag {
                id: 12052,
                name: "skee".into(),
                post_count: 2,
                related_tags: concat!(
                    "male 2 solo 2 skee 2 simple_background 2 shorts 1 bulge 1 ",
                    "clothing 1 genuine_outline 1 gentle_taps 1 hi_res 1 huge_balls 1 ",
                    "huge_peanut 1 hyper 1 hyper_balls 1 hyper_melancholia 1 ",
                    "hyper_peanut 1 limitedvision 1 lizard 1 anthro 1 multi_melancholia 1 ",
                    "multi_peanut 1 peanut 1 peanut_outline 1 reptile 1 scalie 1",
                )
                .into(),
                related_tags_updated_at: "2020-02-24T21:51:39.390-05:00".parse().ok(),
                category: Category::Artist,
                is_locked: false,
                created_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
                updated_at: "2020-03-05T05:49:37.994-05:00".parse().unwrap(),
            }),
        ];

        let actual = client.tag_search(query).collect::<Vec<_>>().await;

        assert_eq!(actual, expected);
    }
}
