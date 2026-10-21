use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use scraper::Selector;

use crate::backend::Client;
use crate::extract::{Element, FromContext, Html, Select};
use crate::frontier::queue::Link;
use crate::handler::Handler;
use crate::request::Tag;
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, TRACING_TARGET};

/// A handler that follows the links of a page, queueing them with a tag.
///
/// By default it follows every `<a href>`; [`select`](Self::select)
/// narrows it to the elements matching a CSS selector, e.g. a pagination
/// link. Which links are crawled is still up to the frontier: its scope,
/// deduplication and depth limits apply. Combine it with other handlers in a
/// tuple to do more with the same page.
///
/// ```
/// use spire::{FollowLinks, Router};
///
/// let router: Router<spire::backend::Noop> = Router::new()
///     .route("listing", FollowLinks::new("listing").select("a.next"))
///     .fallback(FollowLinks::new("page"));
/// ```
#[must_use]
#[derive(Clone)]
pub struct FollowLinks {
    tag: Tag,
    selector: Option<(Arc<str>, Selector)>,
}

impl FollowLinks {
    /// Follows every link of the page, queueing it with `tag`.
    pub fn new(tag: impl Into<Tag>) -> Self {
        Self {
            tag: tag.into(),
            selector: None,
        }
    }

    /// Follows only the `href` of the elements matching `css`.
    ///
    /// # Panics
    ///
    /// Panics if `css` is not a valid CSS selector.
    pub fn select(mut self, css: &str) -> Self {
        let selector = Selector::parse(css)
            .unwrap_or_else(|error| panic!("invalid CSS selector `{css}`: {error}"));
        self.selector = Some((css.into(), selector));
        self
    }

    /// The `href` of every link to follow, as written in the page.
    fn hrefs(&self, page: &Html) -> Vec<String> {
        let Some((_, selector)) = &self.selector else {
            return page.links();
        };
        page.0
            .select(selector)
            .filter_map(|element| element.value().attr("href"))
            .map(str::to_owned)
            .collect()
    }
}

impl fmt::Debug for FollowLinks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FollowLinks")
            .field("tag", &self.tag)
            .field("selector", &self.selector.as_ref().map(|(css, _)| css))
            .finish()
    }
}

impl<C, S> Handler<C, FollowLinks, S> for FollowLinks
where
    C: Client,
    S: Send + Sync + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let follow = async {
            let hrefs = self.hrefs(&Html::from_context(cx, state).await?);
            let links = hrefs
                .into_iter()
                .map(|href| Link::from(href).with_tag(self.tag.clone()));
            let queued = cx.queue().follow_all(links).await?;
            tracing::debug!(target: TRACING_TARGET, queued, tag = %self.tag, "followed links");
            Ok::<_, crate::Error>(())
        };
        follow.await.into_outcome()
    }
}

/// A handler that extracts every `T` of a page, see [`Select`], and stores
/// them in the `T` dataset.
///
/// The dataset must be registered on the crawler. Combine it with other
/// handlers in a tuple to do more with the same page.
///
/// ```
/// # #[cfg(feature = "macros")] {
/// use spire::extract::Select;
/// use spire::{Collect, FollowLinks, Router};
///
/// #[derive(Debug, Select)]
/// #[select("article.product")]
/// struct Product {
///     #[select("h2")]
///     name: String,
/// }
///
/// // Collect the products of a listing, then follow its next page.
/// let router: Router<spire::backend::Noop> = Router::new().route(
///     "listing",
///     (Collect::<Product>::new(), FollowLinks::new("listing").select("a.next")),
/// );
/// # }
/// ```
#[must_use]
pub struct Collect<T> {
    marker: PhantomData<fn() -> T>,
}

impl<T> Collect<T> {
    /// Collects every `T` of the page.
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<T> Default for Collect<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Collect<T> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Collect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Collect")
            .field("item", &std::any::type_name::<T>())
            .finish()
    }
}

impl<C, S, T> Handler<C, Collect<T>, S> for Collect<T>
where
    C: Client,
    S: Send + Sync + 'static,
    T: Select + Send + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let collect = async {
            let data = cx.dataset::<T>()?;
            let Element(items) = Element::<Vec<T>>::from_context(cx, state).await?;
            let collected = items.len();
            data.write_all(items).await?;
            tracing::debug!(target: TRACING_TARGET, collected, "collected items");
            Ok::<_, crate::Error>(())
        };
        collect.await.into_outcome()
    }
}
