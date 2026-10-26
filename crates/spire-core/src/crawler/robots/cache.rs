//! The robots.txt rules of every origin a run has seen.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use http::Uri;
use robotxt::Robots as Rules;
use tokio::time::Instant;

use crate::crawler::Robots;
use crate::frontier::seed::Seed;
use crate::{Result, TRACING_TARGET};

/// Whether robots.txt lets a seed be crawled.
pub(crate) enum Verdict {
    /// It may be crawled.
    Allow(Seed),
    /// It is disallowed, and must be skipped.
    Disallow(Seed),
    /// It waits for its origin's robots.txt.
    Parked,
}

enum State {
    /// robots.txt is being fetched; old rules, if any, apply meanwhile.
    Fetching {
        attempt: u32,
        stale: Option<Arc<Rules>>,
    },
    /// robots.txt is unreachable; everything waits for the retry.
    Blocked { attempt: u32, retry_at: Instant },
    /// The rules apply.
    Ready {
        rules: Arc<Rules>,
        fetched_at: Instant,
    },
}

struct Origin {
    state: State,
    parked: VecDeque<Seed>,
}

/// What a fetched robots.txt changes.
#[derive(Debug, Default)]
pub(crate) struct Fetched {
    /// Seeds that waited for the rules, to admit now.
    pub(crate) released: Vec<Seed>,
    /// The origin's `Crawl-delay`.
    pub(crate) delay: Option<Duration>,
    /// Sitemaps the rules list, to queue, see [`Robots::with_sitemaps`].
    pub(crate) sitemaps: Vec<Seed>,
}

/// The robots.txt rules of every origin, fetched and refreshed as RFC 9309
/// describes. Origins are keyed by scheme, host and port: `http://` and
/// `https://` of a host have separate robots.txt files.
pub(crate) struct RobotsCache {
    config: Robots,
    origins: HashMap<String, Origin>,
    fetches: Vec<(String, Uri)>,
    parked: usize,
}

/// The key a seed's robots.txt is cached under: its lowercase origin.
pub(crate) fn origin(seed: &Seed) -> String {
    let uri = seed.uri();
    let scheme = uri.scheme_str().unwrap_or("http").to_ascii_lowercase();
    let host = uri.host().unwrap_or_default().to_ascii_lowercase();
    let default_port = if scheme == "https" { 443 } else { 80 };
    match uri.port_u16().filter(|port| *port != default_port) {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    }
}

impl RobotsCache {
    pub(crate) fn new(config: Robots) -> Self {
        Self {
            config,
            origins: HashMap::new(),
            fetches: Vec::new(),
            parked: 0,
        }
    }

    fn queue_fetch(fetches: &mut Vec<(String, Uri)>, origin: &str) {
        match format!("{origin}/robots.txt").parse() {
            Ok(url) => fetches.push((origin.to_owned(), url)),
            Err(error) => {
                tracing::warn!(target: TRACING_TARGET, %origin, %error, "invalid robots.txt url");
            }
        }
    }

    fn decide(rules: &Rules, seed: Seed) -> Verdict {
        let path = seed
            .uri()
            .path_and_query()
            .map_or("/", |path| path.as_str());
        if rules.is_allowed(path) {
            Verdict::Allow(seed)
        } else {
            Verdict::Disallow(seed)
        }
    }

    /// Decides whether `seed` may be crawled, parking it while its origin's
    /// robots.txt is fetched or unreachable.
    pub(crate) fn admit(&mut self, seed: Seed, now: Instant) -> Verdict {
        let key = origin(&seed);
        let max_age = self.config.max_age;
        let entry = self.origins.entry(key.clone()).or_insert_with(|| {
            Self::queue_fetch(&mut self.fetches, &key);
            Origin {
                state: State::Fetching {
                    attempt: 0,
                    stale: None,
                },
                parked: VecDeque::new(),
            }
        });

        match &entry.state {
            State::Ready { rules, fetched_at } => {
                let rules = rules.clone();
                if now.duration_since(*fetched_at) >= max_age {
                    entry.state = State::Fetching {
                        attempt: 0,
                        stale: Some(rules.clone()),
                    };
                    Self::queue_fetch(&mut self.fetches, &key);
                }
                Self::decide(&rules, seed)
            }
            State::Fetching {
                stale: Some(rules), ..
            } => Self::decide(rules, seed),
            State::Fetching { stale: None, .. } | State::Blocked { .. } => {
                entry.parked.push_back(seed);
                self.parked += 1;
                Verdict::Parked
            }
        }
    }

    /// Records the result of fetching `origin`'s robots.txt, returning the
    /// seeds to admit again and the `Crawl-delay` to honour.
    pub(crate) fn fetched(&mut self, key: &str, result: Result<Rules>, now: Instant) -> Fetched {
        let Some(entry) = self.origins.get_mut(key) else {
            return Fetched::default();
        };
        let (attempt, stale) = match &entry.state {
            State::Fetching { attempt, stale } => (*attempt, stale.clone()),
            State::Blocked { .. } | State::Ready { .. } => return Fetched::default(),
        };

        // Sitemaps are queued once, from the origin's first rules.
        let mut sitemaps = Vec::new();
        if let (Ok(rules), None, Some(tag)) = (&result, &stale, &self.config.sitemaps) {
            let seeds = rules
                .sitemaps()
                .iter()
                .filter_map(|url| Seed::get(url.as_str()).ok());
            sitemaps.extend(seeds.map(|seed| seed.with_tag(tag.clone())));
        }

        let rules = match (result, stale) {
            (Ok(rules), _) => Arc::new(rules),
            (Err(error), Some(stale)) => {
                tracing::warn!(target: TRACING_TARGET, origin = %key, %error, "robots.txt unreachable, keeping the cached rules");
                stale
            }
            (Err(error), None) if attempt < self.config.retries => {
                let attempt = attempt + 1;
                let retry_at = now + self.config.retry_delay(attempt);
                tracing::info!(target: TRACING_TARGET, origin = %key, %error, attempt, "robots.txt unreachable, retrying");
                entry.state = State::Blocked { attempt, retry_at };
                return Fetched::default();
            }
            (Err(error), None) => {
                tracing::warn!(target: TRACING_TARGET, origin = %key, %error, "robots.txt unreachable, skipping the origin");
                Arc::new(Rules::disallow_all())
            }
        };

        let delay = rules.crawl_delay();
        entry.state = State::Ready {
            rules,
            fetched_at: now,
        };
        let released: Vec<Seed> = entry.parked.drain(..).collect();
        self.parked -= released.len();
        Fetched {
            released,
            delay,
            sitemaps,
        }
    }

    /// Starts fetching robots.txt again for blocked origins due a retry.
    pub(crate) fn retry_due(&mut self, now: Instant) {
        for (key, entry) in &mut self.origins {
            if let State::Blocked { attempt, retry_at } = entry.state
                && retry_at <= now
            {
                entry.state = State::Fetching {
                    attempt,
                    stale: None,
                };
                Self::queue_fetch(&mut self.fetches, key);
            }
        }
    }

    /// Takes the robots.txt fetches to start: origin keys and URLs.
    pub(crate) fn take_fetches(&mut self) -> Vec<(String, Uri)> {
        std::mem::take(&mut self.fetches)
    }

    /// The earliest retry of a blocked origin.
    pub(crate) fn earliest(&self) -> Option<Instant> {
        self.origins
            .values()
            .filter_map(|entry| match entry.state {
                State::Blocked { retry_at, .. } => Some(retry_at),
                State::Fetching { .. } | State::Ready { .. } => None,
            })
            .min()
    }

    /// The number of parked seeds.
    pub(crate) const fn parked(&self) -> usize {
        self.parked
    }

    /// Takes every parked seed, e.g. when the run stops.
    pub(crate) fn drain(&mut self) -> Vec<Seed> {
        self.parked = 0;
        self.origins
            .values_mut()
            .flat_map(|entry| entry.parked.drain(..))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Tag;
    use crate::{Error, ErrorKind};

    fn seed(uri: &str) -> Seed {
        Seed::get(uri).unwrap()
    }

    fn rules(txt: &str) -> Rules {
        Rules::parse(txt.as_bytes(), "bot")
    }

    fn unreachable() -> Result<Rules> {
        Err(Error::new(ErrorKind::Http))
    }

    #[test]
    fn origins_are_scheme_host_and_port() {
        assert_eq!(
            origin(&seed("HTTPS://Example.com:443/a")),
            "https://example.com"
        );
        assert_eq!(origin(&seed("http://example.com/a")), "http://example.com");
        assert_eq!(
            origin(&seed("http://example.com:8080/a")),
            "http://example.com:8080"
        );
    }

    #[test]
    fn parks_until_fetched_then_decides() {
        let now = Instant::now();
        let mut cache = RobotsCache::new(Robots::new("bot"));

        assert!(matches!(
            cache.admit(seed("https://a.com/x"), now),
            Verdict::Parked
        ));
        assert!(matches!(
            cache.admit(seed("https://a.com/private"), now),
            Verdict::Parked
        ));
        assert!(matches!(
            cache.admit(seed("http://a.com/x"), now),
            Verdict::Parked
        ));
        let fetches: Vec<_> = cache
            .take_fetches()
            .into_iter()
            .map(|(_, url)| url.to_string())
            .collect();
        assert_eq!(
            fetches,
            ["https://a.com/robots.txt", "http://a.com/robots.txt"]
        );

        let ready = cache.fetched(
            "https://a.com",
            Ok(rules("User-agent: *\nDisallow: /private\nCrawl-delay: 2")),
            now,
        );
        let delay = Some(Duration::from_secs(2));
        assert_eq!((ready.released.len(), ready.delay), (2, delay));
        assert_eq!(cache.parked(), 1);
        assert!(matches!(
            cache.admit(seed("https://a.com/x"), now),
            Verdict::Allow(_)
        ));
        assert!(matches!(
            cache.admit(seed("https://a.com/private/y"), now),
            Verdict::Disallow(_)
        ));
    }

    #[test]
    fn retries_unreachable_then_disallows() {
        let now = Instant::now();
        let mut cache = RobotsCache::new(Robots::new("bot").with_unreachable_retries(1));
        cache.admit(seed("https://a.com/"), now);
        cache.take_fetches();

        assert!(
            cache
                .fetched("https://a.com", unreachable(), now)
                .released
                .is_empty()
        );
        assert_eq!(cache.earliest(), Some(now + Duration::from_secs(30)));
        cache.retry_due(now + Duration::from_secs(29));
        assert_eq!(cache.take_fetches().len(), 0);
        cache.retry_due(now + Duration::from_secs(30));
        assert_eq!(cache.take_fetches().len(), 1);

        let fetched = cache.fetched("https://a.com", unreachable(), now);
        assert_eq!(fetched.released.len(), 1);
        assert!(matches!(
            cache.admit(seed("https://a.com/x"), now),
            Verdict::Disallow(_)
        ));
    }

    #[test]
    fn refreshes_expired_rules_keeping_them_meanwhile() {
        let now = Instant::now();
        let hour = Duration::from_hours(1);
        let mut cache = RobotsCache::new(Robots::new("bot").with_max_age(hour));
        cache.admit(seed("https://a.com/"), now);
        cache.take_fetches();
        cache.fetched(
            "https://a.com",
            Ok(rules("User-agent: *\nDisallow: /old")),
            now,
        );

        let later = now + hour;
        assert!(matches!(
            cache.admit(seed("https://a.com/old"), later),
            Verdict::Disallow(_)
        ));
        assert_eq!(cache.take_fetches().len(), 1);
        assert!(matches!(
            cache.admit(seed("https://a.com/old"), later),
            Verdict::Disallow(_)
        ));
        assert_eq!(cache.take_fetches().len(), 0);

        // An unreachable refresh keeps the cached rules.
        cache.fetched("https://a.com", unreachable(), later);
        assert!(matches!(
            cache.admit(seed("https://a.com/old"), later),
            Verdict::Disallow(_)
        ));

        cache.admit(seed("https://a.com/old"), later + hour);
        cache.fetched("https://a.com", Ok(rules("")), later + hour);
        assert!(matches!(
            cache.admit(seed("https://a.com/old"), later + hour),
            Verdict::Allow(_)
        ));
    }

    #[test]
    fn queues_listed_sitemaps_once() {
        let txt = "Sitemap: https://a.com/sitemap.xml\nSitemap: https://cdn.a.com/news.xml";
        let now = Instant::now();
        let hour = Duration::from_hours(1);
        let robots = Robots::new("bot")
            .with_max_age(hour)
            .with_sitemaps("sitemap");
        let mut cache = RobotsCache::new(robots);
        cache.admit(seed("https://a.com/"), now);
        cache.take_fetches();

        let fetched = cache.fetched("https://a.com", Ok(rules(txt)), now);
        let sitemaps: Vec<_> = fetched
            .sitemaps
            .iter()
            .map(|seed| (seed.uri().to_string(), seed.tag().map(Tag::to_string)))
            .collect();
        let tag = Some("sitemap".to_owned());
        assert_eq!(
            sitemaps,
            [
                ("https://a.com/sitemap.xml".to_owned(), tag.clone()),
                ("https://cdn.a.com/news.xml".to_owned(), tag),
            ]
        );

        // Refreshed rules do not queue them again.
        cache.admit(seed("https://a.com/"), now + hour);
        let fetched = cache.fetched("https://a.com", Ok(rules(txt)), now + hour);
        assert!(fetched.sitemaps.is_empty());

        // Without `with_sitemaps`, nothing is queued.
        let mut cache = RobotsCache::new(Robots::new("bot"));
        cache.admit(seed("https://a.com/"), now);
        assert!(
            cache
                .fetched("https://a.com", Ok(rules(txt)), now)
                .sitemaps
                .is_empty()
        );
    }
}
