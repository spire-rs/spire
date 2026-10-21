# Middleware and proxies

Middleware runs around a router for every request: before it, to change the
request or answer without fetching; around it, to check the response's head,
which is fetched once and shared with the handlers; and after it, to change
the outcome. `SetHeader` sets a header on every request, or rotates through
several values, and `middleware::from_fn` turns an async function into
middleware. The middleware added last runs first:

```rust,no_run
use std::time::Duration;

use spire::backend::{HttpClient, Rotate};
use spire::backend::reqwest::{Client, Proxy};
use spire::request::http::HeaderValue;
use spire::request::http::header::USER_AGENT;
use spire::worker::middleware::{self, Next, SetHeader};
use spire::worker::{Outcome, WorkerExt};
use spire::{Context, Crawler, Result, Router};

/// Retries pages answering with a captcha, before any handler runs.
async fn captcha<C: spire::backend::Client>(cx: Context<C>, next: Next<C>) -> Result<Outcome> {
    if cx.response().await?.headers().contains_key("x-captcha") {
        return Ok(Outcome::Retry { after: Some(Duration::from_secs(60)) });
    }
    Ok(next.run(cx).await)
}

async fn crawl() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .route("page", || async {})
        .layer(middleware::from_fn(captcha))
        .layer(SetHeader::rotate(USER_AGENT, [
            HeaderValue::from_static("MyBot/1.0 (a)"),
            HeaderValue::from_static("MyBot/1.0 (b)"),
        ]));

    // Proxies are set per client, so `Rotate` cycles through one per proxy.
    let mut proxies = Vec::new();
    for proxy in ["http://proxy-a:8080", "http://proxy-b:8080"] {
        let client = Client::builder().proxy(Proxy::all(proxy)?).build()?;
        proxies.push(HttpClient::from(client));
    }
    let backend = Rotate::new(proxies);

    Crawler::new(backend, router).run().await?;
    Ok(())
}
```
