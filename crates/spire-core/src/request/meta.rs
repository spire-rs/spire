//! Crawl metadata attached to requests and responses.

use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

use crate::identity::Identity;

/// A label that routes a request to its handler.
///
/// Requests without a tag go to the fallback handler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(Cow<'static, str>);

impl Tag {
    /// Creates a tag from a static string without allocating.
    #[must_use]
    pub const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }

    /// Returns the tag as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        Self(Cow::Owned(value.to_owned()))
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A closed set of [`Tag`]s, typically an enum of the kinds of page a crawl
/// visits.
///
/// Deriving it with `#[derive(Tags)]` (the `macros` feature of `spire`)
/// names each unit variant in `snake_case`, so tags are checked by the
/// compiler while plain string tags keep working:
///
/// - `#[tag(rename_all = "...")]` on the enum picks another case, with
///   serde's names: `lowercase`, `UPPERCASE`, `PascalCase`, `camelCase`,
///   `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case` or
///   `SCREAMING-KEBAB-CASE`.
/// - `#[tag(rename = "...")]` on a variant names it outright.
///
/// Persistent frontiers store tags as strings: renaming a tag strands the
/// seeds stored under its old name, unless `rename` keeps the old one.
///
/// Implemented by hand, it looks like this:
///
/// ```
/// use spire_core::request::{Tag, Tags};
///
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum Page {
///     List,
///     Item,
/// }
///
/// impl Page {
///     const fn as_str(self) -> &'static str {
///         match self {
///             Self::List => "list",
///             Self::Item => "item",
///         }
///     }
/// }
///
/// impl From<Page> for Tag {
///     fn from(page: Page) -> Self {
///         Tag::from_static(page.as_str())
///     }
/// }
///
/// impl Tags for Page {
///     const ALL: &'static [Self] = &[Self::List, Self::Item];
/// }
///
/// assert_eq!(Page::from_tag(&Tag::from("item")), Some(Page::Item));
/// assert_eq!(Page::from_tag(&Tag::from("other")), None);
/// ```
pub trait Tags: Copy + Into<Tag> + 'static {
    /// Every tag in the set.
    const ALL: &'static [Self];

    /// Returns the member named `tag`, if any.
    #[must_use]
    fn from_tag(tag: &Tag) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|member| (*member).into() == *tag)
    }
}

/// How often the pages of each tag are crawled again, for a crawl that keeps
/// its data fresh, see [`FrontierExt::revisit`].
///
/// With the `macros` feature, `#[derive(Revisit)]` implements it from a
/// `#[revisit("1d")]` attribute on each variant to revisit.
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::request::{Revisit, Tag, Tags};
///
/// #[derive(Clone, Copy)]
/// enum Page {
///     Listing,
///     About,
/// }
///
/// impl From<Page> for Tag {
///     fn from(page: Page) -> Self {
///         Tag::from_static(match page {
///             Page::Listing => "listing",
///             Page::About => "about",
///         })
///     }
/// }
///
/// impl Tags for Page {
///     const ALL: &'static [Self] = &[Self::Listing, Self::About];
/// }
///
/// impl Revisit for Page {
///     fn interval(self) -> Option<Duration> {
///         match self {
///             Page::Listing => Some(Duration::from_hours(1)),
///             Page::About => None,
///         }
///     }
/// }
/// ```
///
/// [`FrontierExt::revisit`]: crate::frontier::FrontierExt::revisit
pub trait Revisit: Tags {
    /// How long after a page of this tag is crawled it is crawled again;
    /// `None` crawls it once.
    fn interval(self) -> Option<Duration>;
}

/// How many links away from a seed a request is.
///
/// Seeds have depth 0; every followed link is one deeper than its parent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Depth(u32);

impl Depth {
    /// The depth of a seed.
    pub const ROOT: Self = Self(0);

    /// Creates a depth.
    #[must_use]
    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    /// Returns the depth as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the depth of a link followed from this one.
    #[must_use]
    pub const fn child(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// How many times a request was tried before.
///
/// The first try is attempt 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attempt(u32);

impl Attempt {
    /// The first try.
    pub const FIRST: Self = Self(0);

    /// Creates an attempt.
    #[must_use]
    pub const fn new(attempt: u32) -> Self {
        Self(attempt)
    }

    /// Returns the attempt as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the attempt after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Attempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The crawl metadata of a request, stored as one extension: one allocation
/// per request instead of one per value.
#[derive(Debug, Clone, Default)]
pub(crate) struct RequestMeta {
    pub(crate) tag: Option<Tag>,
    pub(crate) depth: Depth,
    pub(crate) attempt: Attempt,
    /// Picked by the crawl when the request starts, after the rest.
    pub(crate) identity: Option<Identity>,
}
