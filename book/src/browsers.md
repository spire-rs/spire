# Browsers

`BrowserBackend` drives real browsers over WebDriver BiDi, with the page's
real status and headers, any request method, and a fresh browser profile per
request. The page is loaded once, by the first extractor that needs it, and
body extractors such as `Html` read it as currently rendered. `Tab` is
the request's tab, to interact with the page; `tab.snapshot()` then parses it
again, including what the interaction changed:

```rust,no_run
use spire::backend::BrowserBackend;
use spire::backend::thirtyfour::By;
use spire::extract::Tab;
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};

async fn rendered(queue: Queue, tab: Tab) -> Result<()> {
    if let Ok(more) = tab.find(By::Css("button.more")).await {
        let _ = more.click().await;
    }
    let page = tab.snapshot().await?;
    println!("{:?} at {:?}", page.title(), tab.current_url().await);
    queue.follow_all(page.links()).await?;
    Ok(())
}

async fn crawl() -> Result<()> {
    let backend = BrowserBackend::builder()
        .remote("http://localhost:4444")
        .max_sessions(4)
        .build()?;

    let router = Router::new().route("page", rendered);
    let crawler = Crawler::new(backend, router)
        .with_seed(Seed::get("https://example.com/")?.with_tag("page"));
    crawler.run().await?;

    // Quit the browser sessions instead of dropping them.
    crawler.backend().close().await;
    Ok(())
}
```

## Running a browser

The backend talks to a WebDriver server that supports WebDriver BiDi, as
current chromedriver and geckodriver do. Start one, for example
`chromedriver --port=4444`, and point the backend at it with `remote`.
Several `remote` endpoints are used in turn. With the `manager` feature of
`spire-thirtyfour`, `BrowserBuilder::managed` downloads and starts the
driver itself instead; the browser must be installed.

Sessions start headless Chrome by default. Other browsers, and browser
flags, take capabilities:

```rust
use spire::backend::BrowserBackend;
use spire::backend::thirtyfour::DesiredCapabilities;

let builder = BrowserBackend::builder()
    .remote("http://localhost:4444")
    .capabilities(DesiredCapabilities::firefox());
```

## Sessions

The backend keeps a pool of up to `max_sessions` browser sessions, and
every request gets a tab in a fresh profile, so no cookies or storage carry
over between requests. `keep_session_state` shares one profile instead, for
example for a crawl that logs in first.

With [identities](identities.md), each identity gets a profile of its own in
every browser it uses: a user context with its proxy, user agent and
headers, which every request from it carries, the page's scripts and images
included. The profile is kept while the identity is in use, so its cookies
and storage carry over between its requests, never to another identity's.
Proxies asking for credentials get those of the identity's proxy URL. A
browser cannot use a SOCKS proxy with credentials, and drivers without a
proxy per profile fail such requests as `Invalid`.

An identity's profile lives in one browser, so with several sessions, its
requests in different browsers keep separate cookies.

Close the backend after the crawl with `close`, which quits the sessions
before going on. A backend dropped without it quits them on background
tasks, which may not finish if the program exits right after.

## Responses

Navigation goes through WebDriver BiDi, so a response has the page's real
status and headers, after redirects, and error statuses are retried or
rejected as over HTTP. The body is the rendered document serialized as HTML,
so its content type is always `text/html`.

Sitemaps and robots.txt should be fetched over HTTP: a browser renders XML
as a page. `Robots::with_backend` fetches robots.txt with an `HttpClient`
while the pages use the browser.
