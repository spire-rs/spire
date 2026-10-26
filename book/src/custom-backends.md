# Custom backends

A backend fetches pages. `HttpClient` and `BrowserBackend` are two; any
other source of pages, such as a local mirror, an API with its own client,
or a service that renders pages for you, is a backend of your own.

## Backend and Client

A `Backend` is shared by the whole crawl and hands out a `Client` per
request, from `connect`: an HTTP client can hand out a handle on one shared
pool, a browser backend a session from its pool. The client then serves its
one request:

- `fetch(&request)` performs the request and returns the response's head:
  its status and headers. It may keep the body for later.
- `body()` returns the body, after `fetch`: the bytes received, or a
  browser page as currently rendered.

Both are called through the request's `Context`, at most once for `fetch`:
the first extractor that needs the response fetches it, and every other
extractor shares it.

```rust,no_run
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use spire::backend::{Backend, Client};
use spire::request::Request;
use spire::response::Response;
use spire::{Crawler, Error, ErrorKind, Result, Router};

/// Serves pages from a directory, e.g. a site mirrored to disk.
#[derive(Clone)]
struct Mirror {
    root: Arc<PathBuf>,
}

/// A request to a [`Mirror`], keeping the page it read.
struct MirrorPage {
    root: Arc<PathBuf>,
    body: Mutex<Option<Bytes>>,
}

impl Backend for Mirror {
    type Client = MirrorPage;

    async fn connect(&self) -> Result<MirrorPage> {
        Ok(MirrorPage {
            root: Arc::clone(&self.root),
            body: Mutex::new(None),
        })
    }
}

impl Client for MirrorPage {
    async fn fetch(&self, request: &Request) -> Result<Response<()>> {
        let path = self.root.join(request.uri().path().trim_start_matches('/'));
        let body = tokio::fs::read(&path).await.map_err(|error| {
            // Not retried: the file will still be missing.
            Error::new(ErrorKind::Rejected)
                .with_message(format!("no page at {}", path.display()))
                .with_source(error)
        })?;
        *self.body.lock().unwrap() = Some(Bytes::from(body));
        Ok(Response::new(()))
    }

    async fn body(&self) -> Result<Bytes> {
        let body = self.body.lock().unwrap().clone();
        body.ok_or_else(|| Error::worker("the page was not fetched yet"))
    }
}
# async fn page() {}

# async fn crawl() -> Result<()> {
let mirror = Mirror { root: Arc::new(PathBuf::from("./mirror")) };
let crawler = Crawler::new(mirror, Router::new().fallback(page));
crawler.run().await?;
# Ok(())
# }
```

## Doing it as the built-in backends do

- **Classify failures** by whether trying again may help: `Http`, `Timeout`
  and `Backend` errors are retried, `Rejected` and `Invalid` are not; see
  [Errors and retries](errors.md).
- **Apply a `StatusPolicy`** to error statuses with `StatusPolicy::check`,
  so they fail the request as over HTTP, with `Retry-After` delays.
- **Record redirects** with `ResponseExt::set_final_uri`, so links resolve
  against the page's real URI.
- **Test it** with handlers, as [Testing](testing.md) does with `MockSite`,
  itself a backend serving pages from memory.
