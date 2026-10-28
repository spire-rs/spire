use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use ::http::Uri;

use crate::frontier::seed::{Fingerprint, Seed};
use crate::frontier::{Ack, Admission, Frontier, NextSeed, Report};
use crate::sync::lock;
use crate::{Result, TRACING_TARGET};

/// URL budgets for [`AvoidTraps`]. These are heuristics, not proof of a trap.
///
/// Budgets apply independently to each origin and first nonempty path
/// segment, e.g. `https://example.com/archive/`. Query variants count as
/// separate URLs. State is in memory and starts fresh on resume.
#[derive(Debug, Clone, Copy)]
pub struct TrapPolicy {
    urls: usize,
    fanout: usize,
    path_depth: usize,
    repetitions: usize,
    random_urls: usize,
}

impl Default for TrapPolicy {
    fn default() -> Self {
        Self {
            urls: 4096,
            fanout: 256,
            path_depth: 32,
            repetitions: 4,
            random_urls: 8,
        }
    }
}

impl TrapPolicy {
    /// Sets the number of distinct request fingerprints offered per prefix,
    /// 4096 by default. Includes candidates refused by inner facades.
    #[must_use]
    pub const fn with_max_urls(mut self, max: usize) -> Self {
        self.urls = max;
        self
    }

    /// Sets the number of distinct URLs offered under one directory within
    /// a prefix, 256 by default. This estimates fan-out from URL structure,
    /// not from a page's DOM. Query variants share their path's directory.
    #[must_use]
    pub const fn with_max_fanout(mut self, max: usize) -> Self {
        self.fanout = max;
        self
    }

    /// Sets the maximum number of nonempty URL path segments, 32 by default.
    /// This is independent of the seed's link-following depth.
    #[must_use]
    pub const fn with_max_path_depth(mut self, max: usize) -> Self {
        self.path_depth = max;
        self
    }

    /// Sets how many times one segment can repeat in a URL, 4 by default.
    #[must_use]
    pub const fn with_max_repetitions(mut self, max: usize) -> Self {
        self.repetitions = max;
        self
    }

    /// Sets the number of URLs containing opaque-looking path segments,
    /// 8 by default. A segment is suspect when it is at least 16 ASCII
    /// alphanumeric characters, mixes letters and digits, and contains at
    /// least 8 distinct characters. Legitimate identifiers can match;
    /// use `usize::MAX` to disable this budget for such sites.
    #[must_use]
    pub const fn with_max_random_urls(mut self, max: usize) -> Self {
        self.random_urls = max;
        self
    }
}

/// A facade that bounds expanding URL branches, see
/// [`FrontierExt::avoid_traps`](crate::frontier::FrontierExt::avoid_traps).
///
/// When a budget is exceeded, the whole origin/path prefix is blocked for
/// this facade's lifetime. Seeds already queued there are skipped on pop
/// and reported inwards as [`Report::Skipped`]. In-flight work can finish.
/// Other prefixes and origins continue. Refusals are logged and counted
/// by [`refused`](Self::refused), separately from crawler request totals.
///
/// This does not inspect page content, execute CSS, or replace robots.txt
/// and deduplication. Keep this facade outside retry/revisit facades so it
/// also checks the seeds they hand out. Snapshots contain pending seeds,
/// not the heuristic state; persistent storage keeps blocked seeds until
/// they are popped and skipped.
#[derive(Debug)]
pub struct AvoidTraps<F> {
    inner: F,
    policy: TrapPolicy,
    prefixes: Mutex<HashMap<String, Prefix>>,
    refused: AtomicUsize,
}

#[derive(Debug, Default)]
struct Prefix {
    seen: HashSet<Fingerprint>,
    directories: HashMap<String, usize>,
    random_urls: usize,
    blocked: Option<Admission>,
}

impl<F> AvoidTraps<F> {
    /// Wraps `inner` with the URL budgets in `policy`.
    pub fn new(inner: F, policy: TrapPolicy) -> Self {
        Self {
            inner,
            policy,
            prefixes: Mutex::new(HashMap::new()),
            refused: AtomicUsize::new(0),
        }
    }

    /// Returns the number of refused pushes and skipped queued seeds.
    #[must_use]
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }

    fn refusal(&self, seed: &Seed) -> Option<Admission> {
        let key = prefix(seed.uri());
        let mut prefixes = lock(&self.prefixes);
        let state = prefixes.entry(key.clone()).or_default();
        if state.blocked.is_some() {
            return state.blocked;
        }
        let fingerprint = seed.fingerprint();
        if state.seen.contains(&fingerprint) {
            return None;
        }
        let path = seed.uri().path();
        let segments: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
        let directory = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        let fanout = state.directories.get(directory).copied().unwrap_or(0);
        let random = segments.iter().any(|segment| opaque(segment));
        let reason = if segments.len() > self.policy.path_depth {
            Some((Admission::PrefixBudgetExceeded, "path depth"))
        } else if segments.iter().any(|segment| {
            segments.iter().filter(|other| *other == segment).count() > self.policy.repetitions
        }) {
            Some((Admission::SuspectedTrap, "repeated path segments"))
        } else if state.seen.len() >= self.policy.urls {
            Some((Admission::PrefixBudgetExceeded, "URLs per prefix"))
        } else if fanout >= self.policy.fanout {
            Some((Admission::PrefixBudgetExceeded, "directory fan-out"))
        } else if random && state.random_urls >= self.policy.random_urls {
            Some((Admission::SuspectedTrap, "opaque path segments"))
        } else {
            None
        };
        if let Some((admission, reason)) = reason {
            state.blocked = Some(admission);
            // Keep only the blocked marker once this branch is closed.
            state.seen.clear();
            state.directories.clear();
            tracing::warn!(target: TRACING_TARGET, prefix = %key, reason, "blocked URL prefix");
            return Some(admission);
        }
        state.seen.insert(fingerprint);
        *state.directories.entry(directory.to_owned()).or_default() += 1;
        state.random_urls += usize::from(random);
        None
    }

    fn is_blocked(&self, seed: &Seed) -> bool {
        lock(&self.prefixes)
            .get(&prefix(seed.uri()))
            .is_some_and(|state| state.blocked.is_some())
    }

    fn count_refusal(&self, seed: &Seed) {
        self.refused.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(target: TRACING_TARGET, uri = %seed.uri(), "refused blocked URL prefix");
    }
}

impl<F: Frontier> Frontier for AvoidTraps<F> {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        if let Some(admission) = self.refusal(&seed) {
            self.count_refusal(&seed);
            return Ok(admission);
        }
        self.inner.push(seed).await
    }

    async fn pop(&self) -> Result<NextSeed> {
        loop {
            match self.inner.pop().await? {
                NextSeed::Ready(seed) if self.refusal(&seed).is_some() => {
                    self.count_refusal(&seed);
                    self.inner.report(Report::Skipped(seed)).await?;
                }
                pop => return Ok(pop),
            }
        }
    }

    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        self.inner.report(report)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        self.inner.snapshot().map(|seeds| {
            seeds
                .into_iter()
                .filter(|seed| !self.is_blocked(seed))
                .collect()
        })
    }
}

fn prefix(uri: &Uri) -> String {
    let origin = url::Url::parse(&uri.to_string())
        .map_or_else(|_| String::new(), |url| url.origin().ascii_serialization());
    let first = uri
        .path()
        .split('/')
        .find(|part| !part.is_empty())
        .unwrap_or_default();
    if first.is_empty() || opaque(first) {
        format!("{origin}/")
    } else {
        format!("{origin}/{first}/")
    }
}

fn opaque(segment: &str) -> bool {
    segment.len() >= 16
        && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && segment.bytes().any(|byte| byte.is_ascii_digit())
        && segment.bytes().any(|byte| byte.is_ascii_alphabetic())
        && segment.bytes().collect::<HashSet<_>>().len() >= 8
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::frontier::{FrontierExt, InMemFrontier};

    fn seed(path: &str) -> Seed {
        Seed::get(format!("https://site.test{path}")).unwrap()
    }

    #[tokio::test]
    async fn blocks_only_the_over_budget_prefix_and_skips_pending_seeds() {
        let frontier = InMemFrontier::fifo()
            .dedup()
            .avoid_traps_with(TrapPolicy::default().with_max_urls(2));
        for path in ["/trap/a", "/trap/b", "/safe/a"] {
            assert!(frontier.push(seed(path)).await.unwrap().is_queued());
        }
        assert_eq!(
            frontier.push(seed("/trap/c")).await.unwrap(),
            Admission::PrefixBudgetExceeded
        );
        assert_eq!(
            frontier.push(seed("/trap/d")).await.unwrap(),
            Admission::PrefixBudgetExceeded
        );
        assert_eq!(frontier.snapshot().unwrap().len(), 1);
        let NextSeed::Ready(safe) = frontier.pop().await.unwrap() else {
            panic!("safe seed remains");
        };
        assert_eq!(safe.uri().path(), "/safe/a");
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        assert_eq!(frontier.refused(), 4);
    }

    #[tokio::test]
    async fn detects_fanout_depth_repetition_and_opaque_segments() {
        let cases = [
            (TrapPolicy::default().with_max_fanout(1), "/pit/a", "/pit/b"),
            (
                TrapPolicy::default().with_max_path_depth(3),
                "/pit/a/b",
                "/pit/a/b/c",
            ),
            (TrapPolicy::default(), "/pit/a/a/a/a", "/pit/a/a/a/a/a"),
            (
                TrapPolicy::default().with_max_random_urls(1),
                "/pit/aB12cD34eF56gH78",
                "/pit/iJ90kL12mN34oP56",
            ),
        ];
        for (index, (policy, allowed, refused)) in cases.into_iter().enumerate() {
            let frontier = InMemFrontier::fifo().avoid_traps_with(policy);
            assert!(frontier.push(seed(allowed)).await.unwrap().is_queued());
            assert_eq!(
                frontier.push(seed(refused)).await.unwrap(),
                if index < 2 {
                    Admission::PrefixBudgetExceeded
                } else {
                    Admission::SuspectedTrap
                },
                "{refused}"
            );
        }
    }

    #[tokio::test]
    async fn repeated_candidates_do_not_spend_the_budget_again() {
        let frontier = InMemFrontier::fifo()
            .dedup()
            .avoid_traps_with(TrapPolicy::default().with_max_urls(1));
        assert!(frontier.push(seed("/pit/a")).await.unwrap().is_queued());
        assert_eq!(
            frontier.push(seed("/pit/a")).await.unwrap(),
            Admission::Duplicate
        );
        assert_eq!(frontier.refused(), 0);
    }

    #[tokio::test]
    async fn concurrent_pushes_cannot_bypass_the_cap() {
        let frontier = Arc::new(
            InMemFrontier::fifo().avoid_traps_with(TrapPolicy::default().with_max_urls(4)),
        );
        let mut tasks = Vec::new();
        for n in 0..20 {
            let frontier = frontier.clone();
            tasks.push(tokio::spawn(async move {
                frontier.push(seed(&format!("/pit/{n}"))).await.unwrap()
            }));
        }
        let mut queued = 0;
        for task in tasks {
            queued += usize::from(task.await.unwrap().is_queued());
        }
        assert_eq!(queued, 4);
        assert_eq!(frontier.refused(), 16);
    }

    #[tokio::test]
    async fn checks_preexisting_storage_on_pop() {
        let stored = InMemFrontier::fifo();
        stored.push(seed("/pit/a")).await.unwrap();
        stored.push(seed("/pit/b")).await.unwrap();
        let frontier = stored.avoid_traps_with(TrapPolicy::default().with_max_urls(1));
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Ready(_)));
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        assert_eq!(frontier.refused(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_blocked_prefix_does_not_escape_through_a_delayed_retry() {
        let frontier = InMemFrontier::fifo()
            .retry(1)
            .avoid_traps_with(TrapPolicy::default().with_max_urls(1));
        frontier.push(seed("/pit/a")).await.unwrap();
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!("seed");
        };
        frontier
            .report(Report::Retry {
                seed: retried,
                after: Some(std::time::Duration::from_secs(1)),
            })
            .await
            .unwrap();
        assert_eq!(
            frontier.push(seed("/pit/b")).await.unwrap(),
            Admission::PrefixBudgetExceeded
        );
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
        assert_eq!(frontier.refused(), 2);
    }

    #[tokio::test]
    async fn opaque_root_segments_share_a_budget_and_real_ids_can_opt_out() {
        let ids = ["aB12cD34eF56gH78", "iJ90kL12mN34oP56"];
        let guarded =
            InMemFrontier::fifo().avoid_traps_with(TrapPolicy::default().with_max_random_urls(1));
        assert!(
            guarded
                .push(seed(&format!("/{}", ids[0])))
                .await
                .unwrap()
                .is_queued()
        );
        assert_eq!(
            guarded.push(seed(&format!("/{}", ids[1]))).await.unwrap(),
            Admission::SuspectedTrap
        );
        let ids_allowed = InMemFrontier::fifo()
            .avoid_traps_with(TrapPolicy::default().with_max_random_urls(usize::MAX));
        for id in ids {
            assert!(
                ids_allowed
                    .push(seed(&format!("/products/{id}")))
                    .await
                    .unwrap()
                    .is_queued()
            );
        }
    }

    #[tokio::test]
    async fn origins_ports_and_path_boundaries_are_independent() {
        let frontier =
            InMemFrontier::fifo().avoid_traps_with(TrapPolicy::default().with_max_urls(1));
        frontier.push(seed("/pit/a")).await.unwrap();
        assert_eq!(
            frontier.push(seed("/pit/b")).await.unwrap(),
            Admission::PrefixBudgetExceeded
        );
        for uri in [
            "https://site.test/pitfall/a",
            "https://other.test/pit/a",
            "http://site.test/pit/a",
            "https://site.test:8443/pit/a",
        ] {
            assert!(
                frontier
                    .push(Seed::get(uri).unwrap())
                    .await
                    .unwrap()
                    .is_queued(),
                "{uri}"
            );
        }
    }
}
