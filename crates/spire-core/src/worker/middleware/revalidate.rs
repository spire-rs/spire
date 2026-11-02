use std::collections::HashMap;
use std::fmt;
use std::future::{Future, ready};
use std::sync::{Arc, Mutex};

use ::http::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use ::http::{HeaderMap, HeaderValue, Method, StatusCode};

use super::{Middleware, Next};
use crate::Result;
use crate::backend::Session;
use crate::context::Context;
use crate::frontier::seed::{Fingerprint, fingerprint};
use crate::sync::lock;
use crate::worker::Outcome;

/// What identifies a page's version, from its response: its `ETag` and
/// `Last-Modified` headers, see [`Revalidate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageVersion {
    /// The page's `ETag`.
    pub etag: Option<HeaderValue>,
    /// The page's `Last-Modified`.
    pub last_modified: Option<HeaderValue>,
}

impl PageVersion {
    /// Reads the version of a response with `headers`, or `None` if it has
    /// neither header.
    #[must_use]
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let version = Self {
            etag: headers.get(ETAG).cloned(),
            last_modified: headers.get(LAST_MODIFIED).cloned(),
        };
        (version != Self::default()).then_some(version)
    }
}

/// Stores the [`PageVersion`]s of the pages a crawl has seen, by the
/// [`Fingerprint`] of their request.
pub trait Validator: Send + Sync + 'static {
    /// Returns the version stored for `request`, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn get(&self, request: Fingerprint)
    -> impl Future<Output = Result<Option<PageVersion>>> + Send;

    /// Stores `version` for `request`, replacing any earlier one.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn set(
        &self,
        request: Fingerprint,
        version: PageVersion,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// [`Validator`] kept in memory, for one run. Clones share the store.
#[derive(Debug, Clone, Default)]
pub struct InMemValidator {
    inner: Arc<Mutex<HashMap<Fingerprint, PageVersion>>>,
}

impl InMemValidator {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Validator for InMemValidator {
    fn get(
        &self,
        request: Fingerprint,
    ) -> impl Future<Output = Result<Option<PageVersion>>> + Send {
        ready(Ok(lock(&self.inner).get(&request).cloned()))
    }

    fn set(
        &self,
        request: Fingerprint,
        version: PageVersion,
    ) -> impl Future<Output = Result<()>> + Send {
        lock(&self.inner).insert(request, version);
        ready(Ok(()))
    }
}

/// A [`Middleware`] that crawls only pages changed since the crawl last saw
/// them, with conditional requests.
///
/// For a `GET` or `HEAD` whose page has a stored [`PageVersion`], it sends
/// `If-None-Match` and `If-Modified-Since`, unless the request has them
/// already. A server answering `304 Not Modified` says the page did not
/// change: the handlers are skipped, the request is done, and the crawl
/// counts it in [`Stats::unchanged`](crate::crawler::Stats::unchanged).
/// Otherwise the handlers run, and the response's version is stored once
/// they continued, so a page whose handlers failed is fetched in full next
/// time.
///
/// A [`Validator`] kept on disk, e.g. in a redb store, makes later runs
/// conditional too. Browsers handle conditional requests themselves, so it
/// is meant for HTTP backends.
///
/// ```
/// use spire_core::backend::Noop;
/// use spire_core::worker::middleware::{InMemValidator, Revalidate};
/// use spire_core::worker::{WorkerExt, from_fn};
/// use spire_core::crawler::Crawler;
/// use spire_core::Context;
///
/// let worker = from_fn(|cx: Context<Noop>| async move { cx.body().await.map(drop) })
///     .layer(Revalidate::new(InMemValidator::new()));
/// let crawler = Crawler::new(Noop, worker);
/// ```
#[derive(Clone)]
pub struct Revalidate<V> {
    validator: V,
}

impl<V> Revalidate<V> {
    /// Revalidates pages with the versions in `validator`.
    pub const fn new(validator: V) -> Self {
        Self { validator }
    }
}

impl<V> fmt::Debug for Revalidate<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Revalidate").finish_non_exhaustive()
    }
}

impl<C, V> Middleware<C> for Revalidate<V>
where
    C: Session,
    V: Validator,
{
    async fn handle(&self, mut cx: Context<C>, next: Next<C>) -> Outcome {
        let request = cx.request();
        if !matches!(*request.method(), Method::GET | Method::HEAD) {
            return next.run(cx).await;
        }
        let key = fingerprint(request.method(), request.uri(), request.body());

        let stored = match self.validator.get(key).await {
            Ok(stored) => stored,
            Err(error) => return Outcome::Fail(error),
        };
        if let Some(stored) = stored {
            let headers = cx.request_mut().headers_mut();
            if let Some(etag) = stored.etag
                && !headers.contains_key(IF_NONE_MATCH)
            {
                headers.insert(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = stored.last_modified
                && !headers.contains_key(IF_MODIFIED_SINCE)
            {
                headers.insert(IF_MODIFIED_SINCE, last_modified);
            }
        }

        let fresh = match cx.response().await {
            Ok(response) if response.status() == StatusCode::NOT_MODIFIED => {
                return Outcome::Continue;
            }
            Ok(response) => PageVersion::from_headers(response.headers()),
            Err(error) => return Outcome::Fail(error),
        };
        let outcome = next.run(cx).await;
        if let (Outcome::Continue, Some(fresh)) = (&outcome, fresh)
            && let Err(error) = self.validator.set(key, fresh).await
        {
            return Outcome::Fail(error);
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use ::http::header::ETAG;

    use super::*;
    use crate::crawler::Crawler;
    use crate::frontier::seed::Seed;
    use crate::testing::{MockPage, MockSession, MockSite};
    use crate::worker::{WorkerExt, from_fn};
    use crate::{Error, Result};

    /// Crawls `site`'s `/` once with `validator`, returning how many times
    /// the handler ran and the crawl's unchanged count.
    async fn crawl(site: &MockSite, validator: &InMemValidator) -> (usize, usize) {
        let runs = Arc::new(Mutex::new(0));
        let worker = {
            let runs = Arc::clone(&runs);
            from_fn(move |cx: Context<MockSession>| {
                let runs = Arc::clone(&runs);
                async move {
                    cx.body().await?;
                    *runs.lock().unwrap() += 1;
                    Ok::<_, Error>(())
                }
            })
        };
        let worker = worker.layer(Revalidate::new(validator.clone()));
        let crawler = Crawler::new(site.clone(), worker)
            .with_seed(Seed::get("https://example.com/").unwrap());
        let stats = crawler.run().await.unwrap();
        let runs = *runs.lock().unwrap();
        (runs, stats.unchanged)
    }

    #[tokio::test]
    async fn unchanged_pages_skip_their_handlers() {
        let page =
            MockPage::html("<p>Hi</p>").with_header(ETAG, HeaderValue::from_static("\"v1\""));
        let site = MockSite::new().page("https://example.com/", page);
        let validator = InMemValidator::new();

        // Fetched in full, and its version stored.
        assert_eq!(crawl(&site, &validator).await, (1, 0));
        let key = Seed::get("https://example.com/").unwrap().fingerprint();
        let stored = validator.get(key).await.unwrap().unwrap();
        assert_eq!(stored.etag.unwrap(), "\"v1\"");

        // Then revalidated: unchanged, so the handler does not run.
        assert_eq!(crawl(&site, &validator).await, (0, 1));
    }

    #[tokio::test]
    async fn changed_pages_and_pages_without_versions_run_their_handlers() -> Result<()> {
        let validator = InMemValidator::new();
        let v1 = MockPage::html("1").with_header(ETAG, HeaderValue::from_static("\"v1\""));
        let v2 = MockPage::html("2").with_header(ETAG, HeaderValue::from_static("\"v2\""));
        crawl(
            &MockSite::new().page("https://example.com/", v1),
            &validator,
        )
        .await;
        assert_eq!(
            crawl(
                &MockSite::new().page("https://example.com/", v2),
                &validator
            )
            .await,
            (1, 0)
        );

        let plain = MockSite::new().page("https://example.com/", MockPage::html("x"));
        let fresh = InMemValidator::new();
        assert_eq!(crawl(&plain, &fresh).await, (1, 0));
        assert_eq!(crawl(&plain, &fresh).await, (1, 0), "nothing to revalidate");
        Ok(())
    }

    #[tokio::test]
    async fn failed_handlers_store_no_version() {
        let page = MockPage::html("x").with_header(ETAG, HeaderValue::from_static("\"v1\""));
        let site = MockSite::new().page("https://example.com/", page);
        let validator = InMemValidator::new();
        let failing =
            from_fn(|_: Context<MockSession>| async { Err::<(), _>(Error::worker("bad page")) })
                .layer(Revalidate::new(validator.clone()));
        let crawler =
            Crawler::new(site, failing).with_seed(Seed::get("https://example.com/").unwrap());
        crawler.run().await.unwrap();

        let key = Seed::get("https://example.com/").unwrap().fingerprint();
        assert_eq!(validator.get(key).await.unwrap(), None);
    }
}
