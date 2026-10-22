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

## Facades

`FrontierExt` adds facades, each wrapping the frontier before it:

```rust
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;

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
| `tag_backoff()` | Pauses a tag when a request asks for it, e.g. on `429` |

A crawler without `with_frontier` uses `InMemFrontier::fifo()` with the
default retries, and no deduplication. Add `dedup` to any crawl that follows
links, or it will crawl pages again whenever they are linked twice.

## Retries

`retry(3)` retries a failed request up to 3 times: after 500 ms, then
doubling the delay up to 30 s, each delay shortened by up to 20% so requests
failing together are not retried together. `RetryPolicy` changes those:

```rust
use std::time::Duration;

use spire::frontier::FrontierExt;
use spire::frontier::facade::RetryPolicy;
use spire::frontier::memory::InMemFrontier;

let policy = RetryPolicy::new(5)
    .with_base_delay(Duration::from_secs(1))
    .with_max_delay(Duration::from_secs(60))
    .with_jitter(50);
let frontier = InMemFrontier::fifo().retry_with(policy);
```

Only errors that may succeed next time are retried: timeouts, connection
errors, and `408`, `429` and `5xx` statuses. Other `4xx` statuses, invalid
requests and failed extractors are not. A handler can ask for a retry itself
with `Outcome::retry()`.

## Backoff

When a site answers `429 Too Many Requests` or `503 Service Unavailable`
with a `Retry-After` header, `tag_backoff` pauses every request with the
same tag for that long, then tries the request again. A handler pauses a tag
itself with `Outcome::backoff(duration)`, and `Outcome::abort()` skips every
further request of its tag.

## Surviving a crash

With the `redb` feature, `RedbStore` keeps the frontier and the seen set in
one file on disk. Reopening the file after a crash resumes where the crawl
stopped:

```rust,no_run
use spire::backend::HttpClient;
use spire::frontier::FrontierExt;
use spire::redb::RedbStore;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};

# async fn crawl(router: Router<<HttpClient as spire::backend::Backend>::Client>) -> Result<()> {
let store = RedbStore::open("crawl.redb").await?;
let frontier = store.frontier().dedup_with(store.seen()).retry(3);

let crawler = Crawler::new(HttpClient::default(), router)
    .with_frontier(frontier)
    // Refused as already seen when the crawl resumes.
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
# Ok(())
# }
```

The frontier queues seeds by priority, then in the order queued. A seed
popped for crawling stays recorded until its outcome is reported, so the
requests in flight at a crash are crawled again: a request may run twice,
but none is lost. Commits are synced to disk at most once a second, so a
crash repeats at most a second of work. `RedbStore::builder().sync_always()`
syncs every commit instead, at a large cost in throughput.

Without redb, `Crawler::snapshot` copies the seeds an in-memory frontier
still holds, for example after a crawl was stopped. With the `serde`
feature, seeds serialize, so they can be saved and seeded into the next
crawl.
