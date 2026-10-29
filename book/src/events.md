# Crawl events

Logs are for people and [`Stats`](observability.md#statistics) are totals.
To watch what a long crawl is doing as it does it, feed a dashboard, or
alert when a host starts failing, subscribe to its events: `events()`
returns a stream of typed `Event`s, from `spire::crawler::event`.

```rust,no_run
use futures::StreamExt;
use spire::backend::ReqwestBackend;
use spire::crawler::event::{Event, OutcomeKind};
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let crawler = Crawler::new(ReqwestBackend::default(), Router::new().fallback(page));

let mut events = crawler.events();
tokio::spawn(async move {
    while let Some(event) = events.next().await {
        match event {
            Event::Handled { request, outcome: OutcomeKind::Fail, error, .. } => {
                eprintln!("{} failed: {}", request.uri, error.unwrap());
            }
            Event::HostPaused { host, duration } => {
                eprintln!("{host} asked for a {duration:?} pause");
            }
            Event::Finished { stats } => {
                println!("done: {} requests", stats.requests);
                break;
            }
            _ => {}
        }
    }
});

crawler.run().await?;
# Ok(())
# }
```

## What a crawl reports

| Event | When |
|---|---|
| `Started` | The run started |
| `RequestStarted` | A request started, before its client connects |
| `Fetched` | Its handlers fetched it: the status and how long the fetch took |
| `Handled` | Its handlers returned: the outcome, the error if it failed, and how long the request took |
| `Retried` | The frontier took it back for another attempt |
| `Failed` | It failed for good: not retryable, or out of retries |
| `Skipped` | It was not crawled: robots.txt disallows it, or its tag was aborted |
| `Cancelled` | It was still in flight when the shutdown grace period ended |
| `HostSlowed` | A throttled host lost concurrency, or its gap widened, see [politeness](politeness.md) |
| `HostPaused` | A host sent `Retry-After`, and nothing starts on it for a while |
| `RobotsFetched` | An origin's robots.txt was fetched, or could not be |
| `Stopping` | The run stops starting requests, and why: a shutdown, a limit, a handler, a failure |
| `Finished` | The run is over, with its `Stats` |
| `Lagged` | This subscriber fell behind and missed some events |

Each request reports `RequestStarted`, a `Fetched` for every fetch its
handlers made, then `Handled`, and `Retried` or `Failed` if it did not
succeed; events about its host follow its `Handled`. Events about a
request carry a `RequestInfo`: its method, URI, tag, depth and attempt, but
not its headers or body, so events stay cheap to copy.

`Fetched` carries the status the server answered with, even when the
backend turned it into an error, such as a `404`; it is `None` only if no
response came at all. It comes when the handler asks for the response, so
a handler that never fetches reports none.

## Slow subscribers

Events are broadcast: every subscriber gets each one, and the crawl never
waits for a subscriber. One that falls behind by more than 1024 events
misses the oldest of them, and gets a `Lagged` with how many in their
place. To keep more, forward events to a channel of your own that buffers
them as you need.

When no one subscribes, no event is built, so a crawl that does not use
them pays nothing for them.

A stream lasts as long as its crawler, across runs; `Finished` marks the
end of each run, so break on it to stop watching after one.

Events sit next to the logs, not in place of them: the same things are
still logged as [Logs and statistics](observability.md) describes.
