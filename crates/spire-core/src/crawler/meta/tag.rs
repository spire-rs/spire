use std::borrow::Cow;
use std::fmt;

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
/// use spire_core::crawler::{Tag, Tags};
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
