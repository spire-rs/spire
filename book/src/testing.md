# Testing handlers

With the `testing` feature, `MockSite` serves fixed pages by URL, so
handlers can be tested without a network. Enable it for tests only:

```toml
[dev-dependencies]
spire = { version = "0.8.0", features = ["testing"] }
```

`MockSite` is a backend, so a whole crawl runs against it, and `visit` runs
one request through a router without crawling what it queues, to check what
the handlers did:

```rust
use spire::dataset::InMemDataset;
use spire::extract::html::Select;
use spire::frontier::seed::Seed;
use spire::testing::{MockPage, MockSite};
use spire::worker::Outcome;
use spire::handler::{Collect, FollowLinks};
use spire::{Result, Router};

#[derive(Debug, Select)]
#[select("article")]
struct Product {
    #[select("h2")]
    name: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let site = MockSite::new().page(
        "https://shop.test/list",
        MockPage::html(r#"<article><h2>Lamp</h2></article><a class="next" href="?page=2">next</a>"#),
    );
    let router = Router::new().route("listing", (
        Collect::<Product>::new(),
        FollowLinks::new("listing").select("a.next"),
    ));

    let visit = site
        .visit(Seed::get("https://shop.test/list")?.with_tag("listing"))
        .with_dataset(InMemDataset::<Product>::new());
    assert!(matches!(visit.run(&router).await, Outcome::Continue));

    let products = visit.dataset::<Product>()?.read_all().await?;
    assert_eq!(products[0].name, "Lamp");
    assert_eq!(visit.queued()[0].uri(), "https://shop.test/list?page=2");
    Ok(())
}
```

Pages are `html`, `text`, `json`, `xml`, a `status`, or a `redirect`, which
is followed as an HTTP client would. URLs without a page get a `404`, and
error statuses fail the fetch as they do over HTTP.

## Behaving like a server

`MockSite` answers as a real server would, so the parts of a crawl that
depend on the server can be tested too:

- **Error statuses** go through the same `StatusPolicy` as the real
  backends: a `404` is rejected, and a `429` or `503` with `Retry-After` is
  retried after it. `with_status_policy(StatusPolicy::Accept)` hands every
  status to the handlers instead.
- **Redirects** with `MockPage::redirect` are followed, and the final URI is
  set, so links resolve as they would over HTTP.
- **Slow pages** with `MockPage::with_latency` take that long to answer,
  e.g. to test timeouts or adaptive host concurrency. With
  `#[tokio::test(start_paused = true)]`, the wait takes no real time.
- **Conditional requests**: a page with an `ETag` or `Last-Modified` header
  answers `304 Not Modified` to a request that still matches it, e.g. to
  test `Revalidate`.

```rust
use std::time::Duration;

use spire::http::http::header::{ETAG, RETRY_AFTER};
use spire::http::http::{HeaderValue, StatusCode};
use spire::testing::{MockPage, MockSite};

let site = MockSite::new()
    .page(
        "https://shop.test/busy",
        MockPage::status(StatusCode::TOO_MANY_REQUESTS)
            .with_header(RETRY_AFTER, HeaderValue::from_static("5")),
    )
    .page(
        "https://shop.test/slow",
        MockPage::html("<p>slow</p>").with_latency(Duration::from_secs(10)),
    )
    .page(
        "https://shop.test/cached",
        MockPage::html("<p>v1</p>").with_header(ETAG, HeaderValue::from_static("\"v1\"")),
    );
```
