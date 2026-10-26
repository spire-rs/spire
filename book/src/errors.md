# Errors and retries

Every request ends with an `Outcome`, which the crawler reports to the
frontier. Failures carry an `Error`, whose kind decides whether the request
is tried again.

## Outcomes

A handler returns anything that converts into an `Outcome`: `()` and
`Ok(())` mean the request is done, and an error fails it.

| Outcome | Means |
|---|---|
| `Continue` | The request is done |
| `Retry { after }` | Try it again, after a delay or the frontier's default |
| `Backoff { scope, duration }` | Pause every request with this tag, then try this one again |
| `Abort(scope)` | Skip every further request with this tag |
| `Stop` | Stop the crawl: requests in flight finish, nothing new starts |
| `Fail(error)` | The request failed: retried if the error is retryable |

Helpers build the common ones: `Outcome::retry()`,
`Outcome::retry_after(delay)`, `Outcome::backoff(duration)` and
`Outcome::abort()`, plus `backoff_tag` and `abort_tag` for another tag.

## Where errors come from

- **A handler** returns an `Error`, through `Result`, or `Outcome::Fail`.
- **An extractor** that cannot extract fails the request before the handler
  runs, with its rejection: an `Html` of a page that cannot be fetched, a
  `Json<T>` of a body that is not a `T`. Wrapping it in `Option` or `Result`
  hands the handler `None` or the error instead.
- **The backend** fails a fetch: a transfer that broke, a timeout, or an
  error status under the default `StatusPolicy`, see [HTTP](http.md).
- **No route** takes a request: it fails.

## Kinds

| `ErrorKind` | Means | Retried |
|---|---|---|
| `Http` | The transfer failed, or the server answered a status worth retrying: `408`, `429`, `5xx` | Yes |
| `Timeout` | The fetch, or the whole request under `with_request_timeout`, took too long | Yes |
| `Backend` | The backend failed, e.g. a browser that would not start | Yes |
| `Rejected` | The server refused the request for good: other `4xx`, too many redirects | No |
| `Invalid` | A malformed URL, request or setting | No |
| `Worker` | A handler or extractor failed | No |
| `Dataset` | Storing or reading an item failed | No |
| `Frontier` | The frontier's storage failed; the crawl stops | No |
| `Cancelled` | The request was cancelled | No |

`Error::worker("...")` is the usual way for a handler to fail; `Error::new`
takes any kind, and `with_source` keeps the cause:

```rust
use spire::extract::Text;
use spire::{Error, ErrorKind, Result};

async fn page(Text(text): Text) -> Result<()> {
    if text.contains("Please try again later") {
        // Retried: the site is busy, not wrong.
        return Err(Error::new(ErrorKind::Http).with_message("soft error page"));
    }
    let count: u32 = text
        .trim()
        .parse()
        .map_err(|error| Error::worker("not a count").with_source(error))?;
    println!("{count}");
    Ok(())
}
```

## Retries

`retry(3)` retries a failed request up to 3 times: after 500 ms, then
doubling the delay up to 30 s, each delay shortened by up to 20% so requests
failing together are not retried together. `RetryPolicy` changes those:

```rust
use std::time::Duration;

use spire::frontier::facade::RetryPolicy;
use spire::frontier::{FrontierExt, InMemFrontier};

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

A request that fails for good, or runs out of retries, is counted in
`Stats::failed` and logged as a warning; see
[Logs and statistics](observability.md).

## Pausing a tag

When a site answers `429 Too Many Requests` or `503 Service Unavailable`
with a `Retry-After` header, the crawler pauses every request to that host
for as long as it asks, then tries the request again; see
[Limits and politeness](politeness.md). A handler pauses a tag itself with
`Outcome::backoff(duration)`, which `tag_backoff` applies to every request
with that tag, and `Outcome::abort()` skips every further request of its
tag.
