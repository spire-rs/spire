use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::Result;
use crate::backend::Backend;

/// A [`Backend`] cycling through several backends, one per request, e.g.
/// HTTP clients behind different proxies, or browser pools with different
/// profiles.
///
/// Every backend must hand out the same type of client. Clones share their
/// place in the cycle.
///
/// # Examples
///
/// ```
/// use spire_core::backend::{Noop, Rotate};
///
/// let backend = Rotate::new([Noop, Noop]);
/// assert_eq!(backend.backends().len(), 2);
/// ```
pub struct Rotate<B> {
    backends: Arc<[B]>,
    next: Arc<AtomicUsize>,
}

impl<B> Rotate<B> {
    /// Cycles through `backends` in order.
    ///
    /// # Panics
    ///
    /// Panics if `backends` is empty.
    #[must_use]
    pub fn new(backends: impl IntoIterator<Item = B>) -> Self {
        let backends: Arc<[B]> = backends.into_iter().collect();
        assert!(!backends.is_empty(), "`Rotate` needs a backend");
        Self {
            backends,
            next: Arc::default(),
        }
    }

    /// Returns the backends, e.g. to close them after a crawl.
    #[must_use]
    pub fn backends(&self) -> &[B] {
        &self.backends
    }
}

impl<B: Backend> Backend for Rotate<B> {
    type Client = B::Client;

    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send {
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        self.backends[index].connect()
    }
}

impl<B> Clone for Rotate<B> {
    fn clone(&self) -> Self {
        Self {
            backends: Arc::clone(&self.backends),
            next: Arc::clone(&self.next),
        }
    }
}

impl<B: fmt::Debug> fmt::Debug for Rotate<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rotate")
            .field("backends", &self.backends)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Crawler;
    use crate::frontier::seed::Seed;
    use crate::testing::{MockPage, MockSite};
    use crate::worker::from_fn;

    #[tokio::test]
    async fn spreads_requests_over_every_backend() {
        let site = || MockSite::new().page("https://example.com/", MockPage::text(""));
        let backend = Rotate::new([site(), site(), site()]);
        let worker = from_fn(|cx: crate::Context<_>| async move { cx.response().await.map(drop) });
        let seeds = (0..6).map(|i| Seed::get(format!("https://example.com/?{i}")).unwrap());

        let crawler = Crawler::new(backend.clone(), worker).with_seeds(seeds);
        assert_eq!(crawler.run().await.unwrap().requests, 6);
        for site in backend.backends() {
            assert_eq!(site.requests().len(), 2);
        }
    }

    #[test]
    #[should_panic(expected = "`Rotate` needs a backend")]
    fn empty_rotations_panic() {
        let _ = Rotate::<MockSite>::new([]);
    }
}
