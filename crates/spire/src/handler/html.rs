use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use scraper::{ElementRef, Selector};

use crate::backend::Client;
use crate::extract::FromContext;
use crate::extract::html::{Element, Html, Select};
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
/// use spire::handler::FollowLinks;
/// use spire::Router;
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
    page_scorer: Option<Arc<PageScorer>>,
    link_scorer: Option<Arc<LinkScorer>>,
}

/// Scores a page, for every link found on it.
type PageScorer = dyn Fn(&Html) -> i32 + Send + Sync;

/// Scores a link from its element.
type LinkScorer = dyn Fn(ElementRef<'_>) -> i32 + Send + Sync;

impl FollowLinks {
    /// Follows every link of the page, queueing it with `tag`.
    pub fn new(tag: impl Into<Tag>) -> Self {
        Self {
            tag: tag.into(),
            selector: None,
            page_scorer: None,
            link_scorer: None,
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

    /// Adds `scorer`'s score of the page to the priority of every link
    /// found on it, scoring the page once, so a frontier that orders by
    /// priority crawls the links of the most relevant pages first.
    ///
    /// It adds to [`score_links`](Self::score_links), and a
    /// [scoring](crate::frontier::FrontierExt::score) frontier adds its own
    /// score on top.
    ///
    /// ```
    /// use spire::frontier::score::Keywords;
    /// use spire::handler::FollowLinks;
    ///
    /// let keywords = Keywords::new(["laptop", "review"]);
    /// let follow = FollowLinks::new("page")
    ///     .score_page(move |page| 10 * keywords.matches(&page.title().unwrap_or_default()));
    /// ```
    pub fn score_page<F>(mut self, scorer: F) -> Self
    where
        F: Fn(&Html) -> i32 + Send + Sync + 'static,
    {
        self.page_scorer = Some(Arc::new(scorer));
        self
    }

    /// Adds `scorer`'s score of every link's element, e.g. by its anchor
    /// text, to the link's priority, so a frontier that orders by priority
    /// crawls the most promising links first.
    ///
    /// It adds to [`score_page`](Self::score_page), and a
    /// [scoring](crate::frontier::FrontierExt::score) frontier adds its own
    /// score on top.
    ///
    /// ```
    /// use spire::frontier::score::Keywords;
    /// use spire::handler::FollowLinks;
    ///
    /// let keywords = Keywords::new(["laptop", "review"]);
    /// let follow = FollowLinks::new("page")
    ///     .score_links(move |link| keywords.matches(&link.text().collect::<String>()));
    /// ```
    pub fn score_links<F>(mut self, scorer: F) -> Self
    where
        F: Fn(ElementRef<'_>) -> i32 + Send + Sync + 'static,
    {
        self.link_scorer = Some(Arc::new(scorer));
        self
    }

    /// Every link to follow, with its `href` as written in the page.
    fn links(&self, page: &Html) -> Vec<Link> {
        let scored = self.page_scorer.is_some() || self.link_scorer.is_some();
        let page_score = self.page_scorer.as_ref().map_or(0, |scorer| scorer(page));
        let link = |(href, element): (&str, ElementRef<'_>)| {
            let link = Link::from(href).with_tag(self.tag.clone());
            if !scored {
                return link;
            }
            let link_score = self
                .link_scorer
                .as_ref()
                .map_or(0, |scorer| scorer(element));
            link.with_priority(page_score.saturating_add(link_score))
        };
        match &self.selector {
            Some((_, selector)) => page
                .0
                .select(selector)
                .filter_map(|element| Some((element.value().attr("href")?, element)))
                .map(link)
                .collect(),
            None => page.anchors().map(link).collect(),
        }
    }
}

impl fmt::Debug for FollowLinks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FollowLinks")
            .field("tag", &self.tag)
            .field("selector", &self.selector.as_ref().map(|(css, _)| css))
            .field("score_page", &self.page_scorer.is_some())
            .field("score_links", &self.link_scorer.is_some())
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
            let links = self.links(&Html::from_context(cx, state).await?);
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
/// use spire::extract::html::Select;
/// use spire::handler::{Collect, FollowLinks};
/// use spire::Router;
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
