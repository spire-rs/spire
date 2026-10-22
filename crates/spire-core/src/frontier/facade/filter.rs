use std::fmt;
use std::future::Future;

use crate::Result;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, Pop, Report};

/// A facade that refuses seeds a predicate rejects, see
/// [`FrontierExt::filter`](crate::frontier::FrontierExt::filter).
pub struct Filter<F, P> {
    inner: F,
    predicate: P,
}

impl<F, P> Filter<F, P> {
    /// Wraps `inner`, queueing only seeds `predicate` accepts.
    pub const fn new(inner: F, predicate: P) -> Self {
        Self { inner, predicate }
    }
}

impl<F: fmt::Debug, P> fmt::Debug for Filter<F, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<F, P> Frontier for Filter<F, P>
where
    F: Frontier,
    P: Fn(&Seed) -> bool + Send + Sync + 'static,
{
    async fn push(&self, seed: Seed) -> Result<Admission> {
        if (self.predicate)(&seed) {
            self.inner.push(seed).await
        } else {
            Ok(Admission::OutOfScope)
        }
    }

    fn pop(&self) -> impl Future<Output = Result<Pop>> + Send {
        self.inner.pop()
    }

    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        self.inner.report(report)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        self.inner.snapshot()
    }
}

/// The hosts a crawl may visit, see
/// [`FrontierExt::allow_hosts`](crate::frontier::FrontierExt::allow_hosts).
#[derive(Debug, Clone)]
pub struct AllowedHosts {
    hosts: Vec<String>,
}

impl AllowedHosts {
    /// Allows `hosts` and their subdomains, compared case-insensitively.
    pub fn new<I>(hosts: I) -> Self
    where
        I: IntoIterator<Item: AsRef<str>>,
    {
        let hosts = hosts
            .into_iter()
            .map(|host| host.as_ref().trim_matches('.').to_ascii_lowercase())
            .collect();
        Self { hosts }
    }

    /// Whether `seed`'s host is allowed.
    #[must_use]
    pub fn allows(&self, seed: &Seed) -> bool {
        let Some(host) = seed.uri().host() else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        self.hosts.iter().any(|allowed| {
            host == *allowed
                || host
                    .strip_suffix(allowed.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::frontier::memory::InMemFrontier;
    use crate::frontier::seed::Seed;
    use crate::frontier::test_util::seed;
    use crate::frontier::{Admission, Frontier, FrontierExt};

    #[tokio::test]
    async fn filter_refuses_rejected_seeds() {
        let frontier = InMemFrontier::fifo().filter(|seed| seed.uri().query().is_none());
        assert!(frontier.push(seed("page")).await.unwrap().is_queued());
        assert_eq!(
            frontier
                .push(Seed::get("https://example.com/?q=1").unwrap())
                .await
                .unwrap(),
            Admission::OutOfScope
        );
    }

    #[tokio::test]
    async fn allow_hosts_matches_subdomains_only() {
        let frontier = InMemFrontier::fifo().allow_hosts(["Example.com"]);
        let admit = |uri: &str| {
            let frontier = &frontier;
            let seed = Seed::get(uri).unwrap();
            async move { frontier.push(seed).await.unwrap() }
        };

        assert!(admit("https://example.com/").await.is_queued());
        assert!(admit("https://docs.EXAMPLE.com/").await.is_queued());
        assert_eq!(
            admit("https://notexample.com/").await,
            Admission::OutOfScope
        );
        assert_eq!(
            admit("https://example.com.evil.org/").await,
            Admission::OutOfScope
        );
    }
}
