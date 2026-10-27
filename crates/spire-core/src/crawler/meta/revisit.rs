use std::time::Duration;

use super::Tags;

/// How often the pages of each tag are crawled again, for a crawl that keeps
/// its data fresh, see [`FrontierExt::revisit`].
///
/// With the `macros` feature, `#[derive(Revisit)]` implements it from a
/// `#[revisit("1d")]` attribute on each variant to revisit.
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::crawler::{Revisit, Tag, Tags};
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
