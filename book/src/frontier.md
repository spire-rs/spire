# The frontier

The frontier holds the seeds waiting to be crawled. It decides their order,
and *facades* on top of it decide which seeds are let in and what happens
when one fails.

## Seeds

A seed is a request waiting to be crawled:

```rust
use spire::frontier::seed::Seed;
use spire::request::http::{HeaderValue, Method};
use spire::request::http::header::ACCEPT;

# fn main() -> spire::Result<()> {
let page = Seed::get("https://example.com/products")?
    .with_tag("listing")
    .with_priority(10);

let search = Seed::new(Method::POST, "https://example.com/search".parse().unwrap())?
    .with_header(ACCEPT, HeaderValue::from_static("application/json"))
    .with_body(r#"{"q": "lamp"}"#);
# Ok(())
# }
```

Seeds are deduplicated by their *fingerprint*: the method, the URI in a
canonical form, and the body. The canonical form lowercases the scheme and
host, drops default ports and the fragment, sorts the query, and removes
tracking parameters such as `utm_source`, so
`HTTPS://Example.com:443/a?b=2&a=1&utm_source=x` and
`https://example.com/a?a=1&b=2` are the same page.

## Order

`InMemFrontier` keeps seeds in memory:

- `InMemFrontier::fifo()` crawls in the order seeds were queued, breadth
  first. This is the default.
- `InMemFrontier::lifo()` crawls the most recent first, depth first.
- `InMemFrontier::priority()` crawls the highest `with_priority` first, then
  in the order queued.

A redb store's frontier orders by priority too, see
[Crash safety with redb](redb.md).

Priorities can be scored for every link, so the crawl reaches the pages
that matter first: see [Best-first crawling](best-first.md).

## Facades

`FrontierExt` adds facades, each wrapping the frontier before it:

```rust
use spire::frontier::{FrontierExt, InMemFrontier};

let frontier = InMemFrontier::fifo()
    .dedup()
    .allow_hosts(["example.com"])
    .filter(|seed| !seed.uri().path().ends_with(".pdf"))
    .max_depth(5)
    .retry(3)
    .tag_backoff();
```

| Facade | Does |
|---|---|
| `dedup()` | Refuses seeds seen before, in memory |
| `dedup_with(seen)` | The same, with another seen set, e.g. `BloomSeen` or on disk |
| `allow_hosts(hosts)` | Refuses seeds of other hosts |
| `filter(predicate)` | Refuses seeds the predicate rejects |
| `max_depth(depth)` | Refuses seeds further than `depth` links from a seed |
| `retry(times)` | Retries failed requests, with exponential backoff |
| `retry_with(policy)` | The same, with a `RetryPolicy` |
| `tag_backoff()` | Pauses a tag when a handler backs off |

A crawler without `with_frontier` uses `InMemFrontier::fifo()` with the
default retries, and no deduplication. Add `dedup` to any crawl that follows
links, or it will crawl pages again whenever they are linked twice.

## Seen sets

`dedup` remembers every fingerprint it has seen, in memory. Long crawls have
other options, through `dedup_with`:

| Seen set | Memory | Exact |
|---|---|---|
| `InMemSeen`, which `dedup()` uses | About 16 bytes per URL | Yes |
| `BloomSeen::new(expected, rate)` | About 1.2 bytes per expected URL at a 1% rate | No: a new URL is mistaken for a seen one, and skipped, at `rate` |
| A redb store's `seen()` | Flat: kept on disk | Yes |

```rust
use spire::frontier::seen::BloomSeen;
use spire::frontier::{FrontierExt, InMemFrontier};

// Ten million URLs in about 12 MB, skipping one new URL in a hundred.
let frontier = InMemFrontier::fifo().dedup_with(BloomSeen::new(10_000_000, 0.01));
```

A Bloom filter past its expected size makes more mistakes; it logs a
warning once.

## Stopping and resuming

A redb store keeps the frontier and the seen set across runs, crashes
included: see [Crash safety with redb](redb.md).

Without it, a stopped crawl's state can be saved and restored by hand.
`Crawler::snapshot` copies the seeds the frontier still holds, including
those waiting for a retry and those the stop cancelled, and `InMemSeen` and
`BloomSeen` have snapshots of their own. With the `serde` feature, both
serialize:

```rust,no_run
use std::sync::Arc;

use spire::backend::HttpClient;
use spire::frontier::seen::InMemSeen;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let seen = Arc::new(InMemSeen::default());
let first = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_frontier(InMemFrontier::fifo().dedup_with(Arc::clone(&seen)))
    .with_max_requests(1000);
first.run().await?;
let (seeds, fingerprints) = (first.snapshot().unwrap_or_default(), seen.snapshot());
// ... save both with serde, and load them in the next run ...

// Restored as is: the queue without facades, the seen set from its
// fingerprints.
let seen = Arc::new(fingerprints.into_iter().collect::<InMemSeen>());
let frontier = seeds.into_iter().collect::<InMemFrontier>().dedup_with(seen);
let next = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_frontier(frontier);
next.run().await?;
# Ok(())
# }
```
