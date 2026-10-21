//! Routing requests to handlers by their [`Tag`], or else their path.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::future::BoxFuture;

mod path;

use self::path::PathPattern;
use crate::handler::Handler;
use crate::request::{RequestExt, Tag, Tags};
use crate::worker::{Outcome, Worker};
use crate::{Context, Error, TRACING_TARGET};

/// A handler bound to its state, with its type erased.
type Route<C> = Arc<dyn ErasedHandler<C>>;

/// An object-safe [`Handler`] bound to its state.
trait ErasedHandler<C>: Send + Sync + 'static {
    fn call<'a>(&'a self, cx: &'a Context<C>) -> BoxFuture<'a, Outcome>;
}

/// A handler and the state it is called with.
struct Bound<H, M, S> {
    handler: H,
    state: S,
    marker: PhantomData<fn() -> M>,
}

impl<C, H, M, S> ErasedHandler<C> for Bound<H, M, S>
where
    H: Handler<C, M, S>,
    S: Send + Sync + 'static,
    M: 'static,
{
    fn call<'a>(&'a self, cx: &'a Context<C>) -> BoxFuture<'a, Outcome> {
        Box::pin(self.handler.call(cx, &self.state))
    }
}

/// Routes requests to handlers by their [`Tag`], or else their path.
///
/// Tagged requests go to the handler registered for their tag. Untagged
/// requests go to the first [path route](Self::route_path) matching their
/// path. Everything else, including tags without a route, goes to the
/// fallback; without a fallback it fails, so a misspelled tag shows up in the
/// crawl's stats.
///
/// Tags can be strings or an enum deriving [`Tags`], which
/// [`require`](Self::require) checks for a route per variant.
///
/// State is given up front with [`with_state`](Self::with_state) and shared
/// by every handler through [`State`](crate::extract::State).
///
/// ```
/// use spire::Router;
/// use spire::extract::Text;
/// use spire::frontier::queue::Queue;
///
/// async fn list(queue: Queue) -> spire::Result<()> {
///     queue.follow("/items/1").await?;
///     Ok(())
/// }
///
/// async fn item(Text(body): Text) {
///     println!("{} bytes", body.len());
/// }
///
/// let router: Router<spire::backend::Noop> = Router::new()
///     .route("list", list)
///     .route("item", item);
/// ```
#[must_use]
pub struct Router<C, S = ()> {
    routes: HashMap<Tag, Route<C>>,
    paths: Vec<(PathPattern, Route<C>)>,
    fallback: Option<Route<C>>,
    state: S,
}

impl<C> Router<C> {
    /// Creates a router without state.
    pub fn new() -> Self {
        Self::with_state(())
    }
}

impl<C, S> Router<C, S> {
    /// Creates a router whose handlers share `state`.
    pub fn with_state(state: S) -> Self {
        Self {
            routes: HashMap::new(),
            paths: Vec::new(),
            fallback: None,
            state,
        }
    }

    /// Routes requests tagged `tag` to `handler`.
    ///
    /// # Panics
    ///
    /// Panics if `tag` already has a route.
    pub fn route<H, M>(self, tag: impl Into<Tag>, handler: H) -> Self
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        let route = self.bind(handler);
        self.insert(tag.into(), route)
    }

    /// Routes untagged requests whose path matches `pattern` to `handler`.
    ///
    /// In `pattern`, `*` matches within one path segment and `**` across
    /// segments; the query is ignored. Path routes are tried in the order
    /// they were added, and only for requests without a tag, so a tag always
    /// wins over the path.
    ///
    /// ```
    /// use spire::Router;
    ///
    /// async fn product() {}
    /// async fn review() {}
    ///
    /// let router: Router<spire::backend::Noop> = Router::new()
    ///     .route_path("/products/*", product)
    ///     .route_path("/products/*/reviews/**", review);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `pattern` does not start with `/` or already has a route.
    pub fn route_path<H, M>(self, pattern: &str, handler: H) -> Self
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        let route = self.bind(handler);
        self.insert_path(PathPattern::new(pattern), route)
    }

    /// Routes requests no tag or path route takes to `handler`.
    pub fn fallback<H, M>(mut self, handler: H) -> Self
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        self.fallback = Some(self.bind(handler));
        self
    }

    /// Checks that every tag of `T` has a route.
    ///
    /// Call it after the routes are added: a forgotten handler then fails
    /// when the router is built rather than as failed requests mid-crawl.
    ///
    /// ```
    /// use spire::Router;
    /// # use spire::request::{Tag, Tags};
    /// # #[derive(Clone, Copy)]
    /// # enum Page { List, Item }
    /// # impl From<Page> for Tag {
    /// #     fn from(page: Page) -> Self {
    /// #         Tag::from_static(match page { Page::List => "list", Page::Item => "item" })
    /// #     }
    /// # }
    /// # impl Tags for Page {
    /// #     const ALL: &'static [Self] = &[Self::List, Self::Item];
    /// # }
    ///
    /// async fn list() {}
    /// async fn item() {}
    ///
    /// // `Page` is `#[derive(Tags)] enum Page { List, Item }`.
    /// let router: Router<spire::backend::Noop> = Router::new()
    ///     .route(Page::List, list)
    ///     .route(Page::Item, item)
    ///     .require::<Page>();
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a tag of `T` has no route, naming every such tag.
    pub fn require<T: Tags>(self) -> Self {
        let missing: Vec<String> = T::ALL
            .iter()
            .map(|&tag| tag.into())
            .filter(|tag| !self.routes.contains_key(tag))
            .map(|tag| format!("`{tag}`"))
            .collect();
        assert!(
            missing.is_empty(),
            "no route for {} {}",
            if missing.len() == 1 { "tag" } else { "tags" },
            missing.join(", ")
        );
        self
    }

    /// Adds the routes of `other`, keeping this router's state.
    ///
    /// Path routes of `other` are tried after this router's.
    ///
    /// # Panics
    ///
    /// Panics if both routers route the same tag or path pattern, or both
    /// have a fallback.
    pub fn merge(mut self, other: Self) -> Self {
        for (tag, route) in other.routes {
            self = self.insert(tag, route);
        }
        for (pattern, route) in other.paths {
            self = self.insert_path(pattern, route);
        }
        if let Some(fallback) = other.fallback {
            assert!(
                self.fallback.replace(fallback).is_none(),
                "both routers have a fallback"
            );
        }
        self
    }

    /// Binds `handler` to the router's state.
    fn bind<H, M>(&self, handler: H) -> Route<C>
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        Arc::new(Bound {
            handler,
            state: self.state.clone(),
            marker: PhantomData,
        })
    }

    fn insert(mut self, tag: Tag, route: Route<C>) -> Self {
        match self.routes.entry(tag) {
            Entry::Occupied(entry) => panic!("tag `{}` already has a route", entry.key()),
            Entry::Vacant(entry) => entry.insert(route),
        };
        self
    }

    fn insert_path(mut self, pattern: PathPattern, route: Route<C>) -> Self {
        assert!(
            self.paths.iter().all(|(other, _)| *other != pattern),
            "path `{}` already has a route",
            pattern.as_str()
        );
        self.paths.push((pattern, route));
        self
    }

    /// Returns the first path route matching `path`.
    fn path_route(&self, path: &str) -> Option<&Route<C>> {
        let mut paths = self.paths.iter();
        let (_, route) = paths.find(|(pattern, _)| pattern.matches(path))?;
        Some(route)
    }

    /// Calls the route for the request: its tag's, else the first matching
    /// path route's for an untagged request, else the fallback.
    async fn dispatch(&self, cx: Context<C>) -> Outcome
    where
        C: Send + Sync + 'static,
    {
        let tag = cx.request().tag();
        tracing::trace!(
            target: TRACING_TARGET,
            tag = tag.map(tracing::field::display),
            "routing request",
        );

        let route = match tag {
            Some(tag) => self.routes.get(tag),
            None => self.path_route(cx.request().uri().path()),
        };
        match route.or(self.fallback.as_ref()) {
            Some(route) => route.call(&cx).await,
            None => unrouted(&cx),
        }
    }
}

/// Fails a request nothing routes.
fn unrouted<C>(cx: &Context<C>) -> Outcome {
    let request = cx.request();
    let message = match request.tag() {
        Some(tag) => format!("no route for tag `{tag}`"),
        None => format!(
            "no path or fallback route for untagged `{}`",
            request.uri().path()
        ),
    };
    Outcome::Fail(Error::worker(message))
}

impl<C> Default for Router<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C, S: Clone> Clone for Router<C, S> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            paths: self.paths.clone(),
            fallback: self.fallback.clone(),
            state: self.state.clone(),
        }
    }
}

impl<C, S> fmt::Debug for Router<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tags: Vec<_> = self.routes.keys().map(Tag::as_str).collect();
        tags.sort_unstable();
        let paths: Vec<_> = self.paths.iter().map(|(pattern, _)| pattern).collect();
        f.debug_struct("Router")
            .field("routes", &tags)
            .field("paths", &paths)
            .field("fallback", &self.fallback.is_some())
            .finish_non_exhaustive()
    }
}

impl<C, S> Worker<C> for Router<C, S>
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send {
        self.dispatch(cx)
    }
}

#[cfg(test)]
mod tests;
