//! Per-host politeness: concurrency, gaps, and slowing down throttled
//! hosts.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::time::Instant;

use crate::TRACING_TARGET;
use crate::crawler::concurrency::{Feedback, HostConcurrency, Limit, Signal};
use crate::crawler::delay::HostDelay;
use crate::frontier::seed::Seed;

/// How a crawl treats every host.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HostPolicy {
    /// Requests in flight per host at most; unlimited if `None`.
    pub(crate) concurrency: Option<HostConcurrency>,
    /// The gap between the starts of two requests to the same host.
    pub(crate) delay: HostDelay,
}

/// What to do with a seed that is allowed to be crawled.
pub(crate) enum Admit {
    /// Start it now.
    Start(Seed),
    /// Its host is busy; it waits in the host's queue.
    Parked,
}

struct Host {
    in_flight: usize,
    /// When the last request to the host started.
    last_start: Option<Instant>,
    /// Starts nothing before then, e.g. for a `Retry-After`.
    paused_until: Option<Instant>,
    /// The policy's delay, its minimum raised by a robots.txt `Crawl-delay`.
    delay: HostDelay,
    /// The current gap between starts, from the delay's minimum up to its
    /// maximum while the host is throttled.
    gap: Duration,
    /// The host's own limit, if limits adapt.
    limit: Limit,
    /// When the host was last slowed down, so the requests already in
    /// flight then slow it down once, not once each.
    slowed: Option<Instant>,
    parked: VecDeque<Seed>,
}

impl Host {
    /// The earliest instant the next request may start: the gap after the
    /// last start, as the gap is now, or the end of a pause.
    fn due(&self) -> Option<Instant> {
        let gap = self.last_start.filter(|_| !self.gap.is_zero());
        let after_gap = gap.map(|last| last + self.gap);
        after_gap.max(self.paused_until)
    }
}

/// The state of every host a run has seen.
///
/// Hosts are keyed by authority, so `http://` and `https://` of the same
/// host share limits: they reach the same server.
pub(crate) struct Hosts {
    policy: HostPolicy,
    /// Whether hosts are tracked: without limits, gaps or throttled hosts,
    /// every host is always free and the bookkeeping is skipped.
    tracked: bool,
    by_key: HashMap<String, Host>,
    parked: usize,
}

/// The key a seed's host is tracked under: its lowercase authority.
fn key(seed: &Seed) -> String {
    seed.uri()
        .authority()
        .map(|authority| authority.as_str().to_ascii_lowercase())
        .unwrap_or_default()
}

impl Hosts {
    /// Tracks hosts if `policy` limits them, or if `delays` may be raised
    /// later, e.g. by a robots.txt `Crawl-delay`.
    pub(crate) fn new(policy: HostPolicy, delays: bool) -> Self {
        let tracked = policy.concurrency.is_some() || !policy.delay.min().is_zero() || delays;
        Self {
            policy,
            tracked,
            by_key: HashMap::new(),
            parked: 0,
        }
    }

    fn host(&mut self, key: &str) -> &mut Host {
        // Looked up first, so the key is only copied for a new host.
        if !self.by_key.contains_key(key) {
            let host = Host {
                in_flight: 0,
                last_start: None,
                paused_until: None,
                delay: self.policy.delay,
                gap: self.policy.delay.min(),
                limit: Limit::new(),
                slowed: None,
                parked: VecDeque::new(),
            };
            self.by_key.insert(key.to_owned(), host);
        }
        self.by_key
            .get_mut(key)
            .unwrap_or_else(|| unreachable!("the host was just inserted"))
    }

    fn is_free(policy: HostPolicy, host: &Host, now: Instant) -> bool {
        let capacity = policy.concurrency.is_none_or(|concurrency| {
            let limit = if concurrency.is_adaptive() {
                host.limit.get()
            } else {
                concurrency.max()
            };
            host.in_flight < limit
        });
        let due = host.due().is_none_or(|at| at <= now);
        capacity && due
    }

    /// Decides whether `seed` starts now or waits for its host.
    pub(crate) fn admit(&mut self, seed: Seed, now: Instant) -> Admit {
        if !self.tracked {
            return Admit::Start(seed);
        }
        let policy = self.policy;
        let host = self.host(&key(&seed));
        if host.parked.is_empty() && Self::is_free(policy, host, now) {
            return Admit::Start(seed);
        }

        host.parked.push_back(seed);
        self.parked += 1;
        Admit::Parked
    }

    /// Takes a parked seed whose host is free.
    pub(crate) fn take_ready(&mut self, now: Instant) -> Option<Seed> {
        let policy = self.policy;
        let host = self
            .by_key
            .values_mut()
            .find(|host| !host.parked.is_empty() && Self::is_free(policy, host, now))?;
        self.parked -= 1;
        host.parked.pop_front()
    }

    /// Records that a request for `seed` started, returning the key to pass
    /// to [`finished`](Self::finished), or `None` if hosts are not tracked.
    pub(crate) fn start(&mut self, seed: &Seed, now: Instant) -> Option<String> {
        if !self.tracked {
            return None;
        }
        let key = key(seed);
        self.started(&key, now);
        Some(key)
    }

    /// Tracks hosts from now on, e.g. once one is throttled, returning the
    /// key of `seed`'s host.
    pub(crate) fn track(&mut self, seed: &Seed) -> String {
        self.tracked = true;
        key(seed)
    }

    /// Records that a request to `key` started.
    pub(crate) fn started(&mut self, key: &str, now: Instant) {
        let host = self.host(key);
        host.in_flight += 1;
        host.last_start = Some(now);
    }

    /// Starts no request to `key` before `until`, e.g. for a `Retry-After`.
    pub(crate) fn pause(&mut self, key: &str, until: Instant) {
        let host = self.host(key);
        if host.paused_until.is_none_or(|at| at < until) {
            tracing::info!(target: TRACING_TARGET, host = key, "pausing host, as it asked");
            host.paused_until = Some(until);
        }
    }

    /// Records that a request to `key` finished at `now`, and slows the
    /// host down if `feedback` says it is throttled, or lets it recover.
    ///
    /// A throttled host first loses concurrency, if its limit adapts and is
    /// above one; then its gap grows, up to the delay's maximum. A host
    /// recovers in the other order: its gap shrinks back to the minimum,
    /// then an adaptive limit grows again.
    pub(crate) fn finished(&mut self, key: &str, feedback: Option<Feedback>, now: Instant) {
        let concurrency = self.policy.concurrency;
        let host = self.host(key);
        host.in_flight = host.in_flight.saturating_sub(1);
        let Some(feedback) = feedback else {
            return;
        };

        let adaptive = concurrency.filter(|concurrency| concurrency.is_adaptive());
        let slow = adaptive.is_some()
            && feedback.signal == Signal::Ok
            && !feedback.unchanged
            && host.limit.observe(feedback.latency);
        if feedback.signal == Signal::Throttled || slow {
            // Requests started before the host was last slowed down were
            // sent at the old pace: they slow it down once, not once each.
            if host.slowed.is_some_and(|at| feedback.started < at) {
                return;
            }
            host.slowed = Some(now);
            if adaptive.is_some() && host.limit.halve() {
                tracing::debug!(target: TRACING_TARGET, host = key, limit = host.limit.get(), "host throttled, lowering its concurrency");
            } else {
                host.gap = host.delay.grow(host.gap);
                tracing::debug!(target: TRACING_TARGET, host = key, gap_ms = host.gap.as_millis(), "host throttled, widening its gap");
            }
        } else if feedback.signal == Signal::Ok {
            if host.gap > host.delay.min() {
                host.gap = host.delay.shrink(host.gap);
            } else if let Some(adaptive) = adaptive {
                host.limit.grow(adaptive.max());
            }
        }
    }

    /// Raises the delay of `key` to at least `delay`, e.g. for a
    /// robots.txt `Crawl-delay`.
    #[cfg(feature = "robots")]
    pub(crate) fn raise_delay(&mut self, key: &str, delay: Duration) {
        let host = self.host(key);
        host.delay.raise(delay);
        host.gap = host.gap.max(delay);
    }

    /// The earliest instant a parked seed's host becomes free by time alone.
    pub(crate) fn earliest(&self) -> Option<Instant> {
        self.by_key
            .values()
            .filter(|host| !host.parked.is_empty())
            .filter_map(Host::due)
            .min()
    }

    /// The number of parked seeds.
    pub(crate) const fn parked(&self) -> usize {
        self.parked
    }

    /// Takes every parked seed, e.g. when the run stops.
    pub(crate) fn drain(&mut self) -> Vec<Seed> {
        self.parked = 0;
        self.by_key
            .values_mut()
            .flat_map(|host| host.parked.drain(..))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(uri: &str) -> Seed {
        Seed::get(uri).unwrap()
    }

    fn policy(max: usize, delay: u64) -> HostPolicy {
        HostPolicy {
            concurrency: (max > 0).then(|| HostConcurrency::fixed(max)),
            delay: Duration::from_secs(delay).into(),
        }
    }

    #[test]
    fn limits_concurrency_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(1, 0), false);

        assert!(matches!(
            hosts.admit(seed("https://a.com/1"), now),
            Admit::Start(_)
        ));
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("http://A.com/2"), now),
            Admit::Parked
        ));
        assert!(matches!(
            hosts.admit(seed("https://b.com/"), now),
            Admit::Start(_)
        ));
        assert!(hosts.take_ready(now).is_none());

        hosts.finished("a.com", None, now);
        assert_eq!(hosts.take_ready(now).unwrap().uri().path(), "/2");
        assert_eq!(hosts.parked(), 0);
    }

    #[test]
    fn spaces_out_requests_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 2), false);
        hosts.started("a.com", now);

        assert!(matches!(
            hosts.admit(seed("https://a.com/2"), now),
            Admit::Parked
        ));
        assert_eq!(hosts.earliest(), Some(now + Duration::from_secs(2)));
        assert!(hosts.take_ready(now + Duration::from_secs(1)).is_none());
        assert!(hosts.take_ready(now + Duration::from_secs(2)).is_some());
    }

    #[test]
    #[cfg(feature = "robots")]
    fn raised_delay_applies_to_the_next_start() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 1), false);
        hosts.raise_delay("a.com", Duration::from_secs(5));
        hosts.started("a.com", now);

        assert!(matches!(
            hosts.admit(seed("https://a.com/"), now),
            Admit::Parked
        ));
        assert_eq!(hosts.earliest(), Some(now + Duration::from_secs(5)));
    }

    #[test]
    fn untracked_hosts_start_everything_at_once() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 0), false);
        for _ in 0..3 {
            let seed = seed("https://a.com/");
            assert!(hosts.start(&seed, now).is_none());
            assert!(matches!(hosts.admit(seed, now), Admit::Start(_)));
        }
        assert_eq!((hosts.parked(), hosts.earliest()), (0, None));

        // A delay that may be raised later keeps hosts tracked.
        let mut hosts = Hosts::new(policy(0, 0), true);
        assert_eq!(
            hosts.start(&seed("https://a.com/"), now).as_deref(),
            Some("a.com")
        );
    }

    #[test]
    fn adaptive_limits_gate_each_host() {
        use crate::crawler::concurrency::Signal;

        let now = Instant::now();
        let policy = HostPolicy {
            concurrency: Some(HostConcurrency::adaptive(4)),
            delay: Duration::ZERO.into(),
        };
        let mut hosts = Hosts::new(policy, false);

        // A host starts at one request at a time.
        let first = seed("https://a.com/1");
        assert!(matches!(hosts.admit(first.clone(), now), Admit::Start(_)));
        hosts.start(&first, now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/2"), now),
            Admit::Parked
        ));
        assert!(matches!(
            hosts.admit(seed("https://b.com/"), now),
            Admit::Start(_)
        ));

        // A fast success raises it to two: the parked seed and one more
        // start, and a fourth waits.
        let fast = Feedback {
            started: now,
            latency: Some(Duration::from_millis(100)),
            unchanged: false,
            signal: Signal::Ok,
        };
        hosts.finished("a.com", Some(fast), now);
        assert_eq!(hosts.by_key["a.com"].limit.get(), 2);
        assert_eq!(hosts.take_ready(now).unwrap().uri().path(), "/2");
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/3"), now),
            Admit::Start(_)
        ));
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/4"), now),
            Admit::Parked
        ));
    }

    /// Finishes a request to `a.com` started at `started`, at `now`.
    fn finish(hosts: &mut Hosts, signal: Signal, started: Instant, now: Instant) {
        let feedback = Feedback {
            started,
            latency: Some(Duration::from_millis(100)),
            unchanged: false,
            signal,
        };
        hosts.finished("a.com", Some(feedback), now);
    }

    fn gap(hosts: &Hosts) -> Duration {
        hosts.by_key["a.com"].gap
    }

    #[test]
    fn throttled_hosts_widen_their_gap_and_recover() {
        use crate::crawler::concurrency::Signal;

        let ms = Duration::from_millis;
        let mut hosts = Hosts::new(
            HostPolicy {
                concurrency: Some(HostConcurrency::fixed(1)),
                delay: (ms(200)..=ms(1000)).into(),
            },
            false,
        );
        let mut now = Instant::now();
        hosts.started("a.com", now);

        // Each round of throttling doubles the gap, up to the maximum...
        let mut gaps = Vec::new();
        for _ in 0..4 {
            let started = now;
            now += Duration::from_secs(1);
            finish(&mut hosts, Signal::Throttled, started, now);
            gaps.push(gap(&hosts));
        }
        assert_eq!(gaps, [ms(400), ms(800), ms(1000), ms(1000)]);

        // ...but requests from the same round count once.
        finish(&mut hosts, Signal::Throttled, now - ms(1), now);
        assert_eq!(gap(&hosts), ms(1000));

        // Successes halve it back to the minimum, and other failures leave
        // it alone.
        finish(&mut hosts, Signal::Failed, now, now);
        assert_eq!(gap(&hosts), ms(1000));
        gaps.clear();
        for _ in 0..3 {
            finish(&mut hosts, Signal::Ok, now, now);
            gaps.push(gap(&hosts));
        }
        assert_eq!(gaps, [ms(500), ms(250), ms(200)]);
    }

    #[test]
    fn adaptive_hosts_lose_concurrency_before_their_gap_widens() {
        use crate::crawler::concurrency::Signal;

        let mut hosts = Hosts::new(
            HostPolicy {
                concurrency: Some(HostConcurrency::adaptive(4)),
                delay: HostDelay::default(),
            },
            false,
        );
        let mut now = Instant::now();
        hosts.started("a.com", now);
        for _ in 0..6 {
            finish(&mut hosts, Signal::Ok, now, now);
        }
        assert_eq!(hosts.by_key["a.com"].limit.get(), 4);

        // 4, 2, 1 at once, then a gap of 1 s, then 2 s.
        let mut steps = Vec::new();
        for _ in 0..4 {
            let started = now;
            now += Duration::from_secs(1);
            finish(&mut hosts, Signal::Throttled, started, now);
            steps.push((hosts.by_key["a.com"].limit.get(), gap(&hosts).as_secs()));
        }
        assert_eq!(steps, [(2, 0), (1, 0), (1, 1), (1, 2)]);

        // Recovering, the gap closes before the limit grows.
        steps.clear();
        for _ in 0..4 {
            finish(&mut hosts, Signal::Ok, now, now);
            steps.push((hosts.by_key["a.com"].limit.get(), gap(&hosts).as_secs()));
        }
        assert_eq!(steps, [(1, 1), (1, 0), (2, 0), (2, 0)]);
    }

    #[test]
    fn unchanged_responses_do_not_set_the_best_time() {
        use crate::crawler::concurrency::Signal;

        let mut hosts = Hosts::new(
            HostPolicy {
                concurrency: Some(HostConcurrency::adaptive(4)),
                delay: HostDelay::default(),
            },
            false,
        );
        let now = Instant::now();
        let response = |latency_ms, unchanged| Feedback {
            started: now,
            latency: Some(Duration::from_millis(latency_ms)),
            unchanged,
            signal: Signal::Ok,
        };
        hosts.finished("a.com", Some(response(100, false)), now);
        for _ in 0..5 {
            hosts.finished("a.com", Some(response(10, true)), now);
        }
        let limit = hosts.by_key["a.com"].limit.get();

        // 150 ms is within twice the 100 ms best: a quick 304 did not lower
        // it, so the full response is not slow.
        hosts.finished("a.com", Some(response(150, false)), now);
        assert!(hosts.by_key["a.com"].limit.get() >= limit);
    }
}
