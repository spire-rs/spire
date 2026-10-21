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

## Adaptive delays

A fixed delay is either too slow for a fast site or too fast for a slow one.
`HostDelay::adaptive` gives each host its own delay instead, adapted to how
fast it answers:

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::crawler::HostDelay;
use spire::{Crawler, Router};
# async fn page() {}

let delay = HostDelay::adaptive()
    .with_target_concurrency(2)
    .with_bounds(Duration::from_millis(100), Duration::from_secs(30));
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
    .with_host_concurrency(4)
    .with_host_delay(delay);
```

- Each host starts at a 1 s delay. After every response, its delay moves
  halfway towards the response time divided by the *target concurrency*:
  the number of requests to aim for in flight per host, 1 by default. A host
  answering in 400 ms with a target of 2 settles on 200 ms between requests.
- A failed request never lowers the delay, so a server failing fast does not
  look fast. A `429`, a `503` or a timeout doubles it.
- The response time is how long the fetch took, not the handlers, so slow
  parsing does not slow the crawl down.
- Delays stay within the bounds, 0 to 60 s by default, and never go below a
  robots.txt `Crawl-delay`. `with_host_concurrency` still caps the requests
  in flight.

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

## Statistics

`run` returns `Stats`: how many requests were made, succeeded, failed,
retried, skipped and cancelled, and how long the crawl took. `stats()`
returns them while the crawl runs, for example to report progress.
