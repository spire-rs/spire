# Limits and politeness

A crawler can easily overload a site, or run for longer than intended.
Spire bounds both.

## Concurrency

`with_concurrency(n)` runs up to `n` requests at once, 8 by default. On top
of it, limits per host keep a single site from taking them all:

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::{Crawler, Router};
# async fn page() {}
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_concurrency(32)
    // At most 2 requests in flight to the same host,
    .with_host_concurrency(2)
    // and at least half a second between starting two of them.
    .with_host_delay(Duration::from_millis(500));
```

Hosts are told apart by their authority, so `http://` and `https://` of the
same host share their limits. A request whose host is busy waits without
holding up requests to other hosts.

## Slowing down

A host is *throttled* when it answers `429 Too Many Requests` or `503
Service Unavailable`, or when a request to it times out. Every crawl slows
a throttled host down, without being asked:

- A `Retry-After` header pauses the host for exactly as long as it asks: no
  request starts to it until then, whatever its tag, and it resumes at the
  same pace after.
- Once its concurrency cannot go lower, each round of throttling doubles the
  gap between its requests: from the delay's minimum, or one second without
  one, up to a maximum of one minute. Each success halves it back.

`with_host_delay` takes the gap's range. A `Duration` is its minimum, with
the one-minute maximum; a range sets both ends, and the same value at both
keeps the gap fixed:

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::{Crawler, Router};
# async fn page() {}
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    // At least 200 ms between requests, up to 5 minutes for a struggling host.
    .with_host_delay(Duration::from_millis(200)..=Duration::from_secs(300));
```

## Adaptive concurrency

A fixed limit is either too timid for a fast site or too much for a slow
one. `HostConcurrency::adaptive` gives each host its own limit instead, up
to a maximum, adapted to how it responds:

```rust,no_run
use spire::backend::HttpClient;
use spire::crawler::HostConcurrency;
use spire::{Crawler, Router};
# async fn page() {}

let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_host_concurrency(HostConcurrency::adaptive(8));
```

- Each host starts at one request at a time. Its limit grows by one for
  every round of responses that come back about as fast as the host's best:
  one more after one success at a limit of one, after two at two, and so on,
  up to the maximum.
- It halves when the host asks to slow down, with a `429` or a `503`, when a
  request times out, or when responses take more than twice the host's best
  time, which is how an overloaded server shows before it fails. It halves
  at most once per round, never below one. Other failures leave it alone.
- Response times are how long the fetch took, not the handlers, so slow
  parsing does not slow the crawl down.
- At a limit of one, a throttled host's gap widens instead, as above.
  Recovering, its gap closes first, then its limit grows again.
- A robots.txt `Crawl-delay` raises a host's minimum gap, whatever the limit
  allows.

## robots.txt

With the `robots` feature, `with_robots` fetches robots.txt once per origin
(scheme, host and port) before its first request, skips what it disallows
for your user agent, and honours its `Crawl-delay`:

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::crawler::Robots;
use spire::{Crawler, Router};
# async fn page() {}

let robots = Robots::new("MyBot")
    // Kept for 6 hours instead of the default 24, then fetched again.
    .with_max_age(Duration::from_secs(6 * 60 * 60));
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_robots(robots);
```

Requests robots.txt disallows are reported as skipped. A missing
robots.txt, answering a `4xx` other than `429`, allows everything. One that
cannot be reached, answering a `5xx` or `429` or failing to connect,
disallows the origin while it is retried. Once its retries are spent, 3 by
default and set with `with_unreachable_retries`, the origin's requests are
skipped. By default, robots.txt is fetched through the crawl's own backend;
`with_backend` fetches it through another, for example plain HTTP while
the pages are crawled with a browser. `with_sitemaps` queues the sitemaps it
lists, see [Sitemaps](sitemaps.md).

The user agent is only matched against robots.txt. To send it with every
request, set it on the backend, or with the `SetHeader` middleware.

## Budgets

| Setting | Stops |
|---|---|
| `with_max_requests(n)` | The crawl, once `n` requests have started, counting every retry |
| `with_time_limit(limit)` | The crawl, once it has run for `limit` |
| `with_request_timeout(timeout)` | One request, failing it, if it takes longer than `timeout` to fetch and handle |
| `with_shutdown_grace(grace)` | Requests still in flight `grace` after the crawl was told to stop |

When the crawl stops, for a limit, a handler's `Outcome::Stop` or a
shutdown, nothing new starts and the requests in flight finish, unless a
grace period cancels them first.

## Shutting down

`shutdown_token` returns a token that stops the crawl when cancelled, for
example on Ctrl-C:

```rust,no_run
# use spire::backend::HttpClient;
# use spire::{Crawler, Result, Router};
# async fn page() {}
# async fn crawl() -> Result<()> {
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page));

let token = crawler.shutdown_token();
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.ok();
    token.cancel();
});

let stats = crawler.run().await?;
# Ok(())
# }
```

How far a crawl got is in its statistics, see
[Logs and statistics](observability.md).
