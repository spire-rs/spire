# Custom backends

A backend fetches pages. `ReqwestBackend` and `ThirtyfourBackend` are two; any
other source of pages, such as a local mirror, an API with its own client,
or a service that renders pages for you, is a backend of your own.

## Backend and Session

A `Backend` is shared by the whole crawl and hands out a `Session` per
request, from `connect`: an HTTP client can hand out a handle on one shared
pool, a browser backend a session from its pool. The client then serves its
one request:

- `fetch(&request)` performs the request and returns a complete
  `Response<Bytes>`: status, headers, final URI and body together.
- `document()` optionally exposes a live `Document` capability. HTTP
  sessions use the default `None`; browser sessions can expose DOM
  observations through this separate trait.

`Context` caches the first successful fetch, including the body. A failed
fetch can be tried again. Later extractors share the same response snapshot;
queries through `Document` observe the live page separately.

```rust,no_run
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use spire::backend::{Backend, Session};
use spire::http::{Request, Response};
use spire::{Crawler, Error, ErrorKind, Result, Router};

/// Serves pages from a directory, e.g. a site mirrored to disk.
#[derive(Clone)]
struct Mirror {
    root: Arc<PathBuf>,
}

/// A session that reads pages from a [`Mirror`].
struct MirrorPage {
    root: Arc<PathBuf>,
}

impl Backend for Mirror {
    type Session = MirrorPage;

    async fn connect(&self) -> Result<MirrorPage> {
        Ok(MirrorPage {
            root: Arc::clone(&self.root),
        })
    }
}

impl Session for MirrorPage {
    async fn fetch(&self, request: &Request) -> Result<Response> {
        let path = self.root.join(request.uri().path().trim_start_matches('/'));
        let body = tokio::fs::read(&path).await.map_err(|error| {
            // Not retried: the file will still be missing.
            Error::new(ErrorKind::Rejected)
                .with_message(format!("no page at {}", path.display()))
                .with_source(error)
        })?;
        Ok(Response::new(Bytes::from(body)))
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
- **Test it** with handlers, as [Testing handlers](testing.md) does with `MockSite`,
  itself a backend serving pages from memory.
