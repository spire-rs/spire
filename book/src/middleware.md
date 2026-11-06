# Middleware

Middleware runs around a router for every request, to do what every handler
would otherwise repeat: set headers, skip some URLs, check every response, or
change what the handlers decided. `layer` wraps a router, or any worker, in
middleware.

## Writing middleware

`middleware::from_fn` turns an async function of the request's `Context` and
the `Next` worker into middleware. It can act at three moments:

- **Before** passing the request on, it can change the request, or answer
  without fetching it at all.
- **Around** the handlers, it can check the response's head. The response is
  fetched once and shared with the handlers, so this costs no extra request.
- **After** the handlers, it can change the `Outcome` they returned.

```rust,no_run
use std::time::Duration;

use spire::backend::{ReqwestBackend, Session};
use spire::http::http::HeaderValue;
use spire::http::http::header::ACCEPT;
use spire::worker::middleware::{self, Next};
use spire::worker::{Outcome, WorkerExt};
use spire::{Context, Crawler, Result, Router};

async fn guard<C: Session>(mut cx: Context<C>, next: Next<C>) -> Result<Outcome> {
    // Before: skip PDFs without fetching them, and ask the API for JSON.
    let path = cx.request().uri().path().to_owned();
    if path.ends_with(".pdf") {
        return Ok(Outcome::Continue);
    }
    if path.starts_with("/api/") {
        let accept = HeaderValue::from_static("application/json");
        cx.request_mut().headers_mut().insert(ACCEPT, accept);
    }

    // Around: retry pages that answer with a captcha, before any handler runs.
    if cx.response().await?.headers().contains_key("x-captcha") {
        return Ok(Outcome::Retry { after: Some(Duration::from_secs(60)) });
    }

    // After: a page that failed is not worth retrying in this crawl.
    Ok(match next.run(cx).await {
        Outcome::Fail(error) => {
            eprintln!("skipping {path}: {error}");
            Outcome::Continue
        }
        outcome => outcome,
    })
}

async fn crawl() -> Result<()> {
    let router = Router::new()
        .route("page", || async {})
        .layer(middleware::from_fn(guard));

    Crawler::new(ReqwestBackend::default(), router).run().await?;
    Ok(())
}
```

Change the request before reading the response: the first extractor or
middleware that reads it fetches the page, with the request as it is then.

## Order

Each `layer` wraps everything before it, so the middleware added last runs
first, and sees the outcome last:

```rust
use spire::Router;
use spire::backend::Noop;
use spire::http::http::HeaderValue;
use spire::http::http::header::{ACCEPT_LANGUAGE, USER_AGENT};
use spire::worker::WorkerExt;
use spire::worker::middleware::SetHeader;

let router = Router::<Noop>::new()
    .route("page", || async {})
    // Runs second, after the user agent is set.
    .layer(SetHeader::new(ACCEPT_LANGUAGE, HeaderValue::from_static("en")))
    // Runs first.
    .layer(SetHeader::new(USER_AGENT, HeaderValue::from_static("MyBot/1.0")));
```

## Built-in middleware

| Middleware | Does | See |
|---|---|---|
| `SetHeader` | Sets a header on every request that lacks it, e.g. a contact address in `From` | |
| `Revalidate` | Sends conditional requests, and skips the handlers of pages that did not change | [Recrawls](recrawls.md) |

Headers that say who a request comes from, such as a user agent that should
match a proxy, belong to an identity instead, which keeps them together with
its proxy and cookies; see [Identities](identities.md).
[Objectives](objectives.md) wrap a router in the same way, with
`with_objective`.

## Reusable middleware

For middleware with its own configuration, implement `Middleware` on a type:

```rust
use std::future::Future;

use spire::Context;
use spire::worker::Outcome;
use spire::worker::middleware::{Middleware, Next};

/// Skips every URL containing one of `paths`, without fetching it.
struct SkipPaths {
    paths: Vec<String>,
}

impl<C: Send + 'static> Middleware<C> for SkipPaths {
    fn handle(&self, cx: Context<C>, next: Next<C>) -> impl Future<Output = Outcome> + Send {
        let path = cx.request().uri().path();
        let skip = self.paths.iter().any(|skipped| path.contains(skipped.as_str()));
        async move {
            if skip {
                Outcome::Continue
            } else {
                next.run(cx).await
            }
        }
    }
}
```
