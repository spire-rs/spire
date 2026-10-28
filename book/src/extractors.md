# Extractors

Extractors are a handler's arguments. They borrow the request's context, so
any of them may read the response: the first that needs it fetches it, and
the others share it.

| Extractor | Gives |
|---|---|
| `Uri` | The request's URI |
| `Query<T>` | The request URI's query string, deserialized |
| `Tag`, or an enum deriving `Tags` | The request's tag |
| `Depth`, `Attempt` | How many links from a seed it is, and which try this is |
| `Queue` | The queue to follow links through, see [Following links](links.md) |
| `Data<T>` | The dataset for items of type `T` |
| `State<T>` | The router's state, or a part of it, see [State](state.md) |
| `Head` | The response's status, headers, and final URI after redirects |
| `Body` | The response body, as bytes |
| `Text` | The body as text |
| `Json<T>` | The body deserialized as JSON |
| `html::Html` | The body parsed as HTML (`html` feature), see [HTML](html.md) |
| `html::Links` | The page's links, resolved, with their `rel` and text (`html` feature), see [Following links](links.md#the-links-of-a-page) |
| `html::Element<T>` | A `T: Select` extracted from the HTML (`html` feature) |
| `html::Meta` | The page's title, description, canonical URL, language, robots, Open Graph and Twitter fields (`html` feature) |
| `html::JsonLd<T>` | The page's JSON-LD structured data (`html` feature) |
| `html::Markdown` | The page's main content as Markdown (`markdown` feature), see [HTML](html.md#markdown) |
| `sitemap::Sitemap` | The body parsed as a sitemap (`sitemap` feature), see [Sitemaps](sitemaps.md) |
| `ThirtyfourTab` | The browser's tab, to interact with the page (`thirtyfour` feature), see [Browsers](browsers.md) |

Wrapping an extractor in `Option` or `Result` makes it infallible: the
handler runs with `None` or the error instead of failing the request.

## The response head and the query

`Head` gives the response's status, headers and the URI it was served from,
for example to read `Last-Modified`, or a status the backend's
`StatusPolicy::Accept` let through. `Query<T>` deserializes the request
URI's query string, for example the current page of a paginated API:

```rust
use serde::Deserialize;
use spire::Result;
use spire::extract::{Head, Query};
use spire::frontier::queue::Queue;

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    page: u32,
}

async fn listing(head: Head, Query(Page { page }): Query<Page>, queue: Queue) -> Result<()> {
    println!("page {page}: {} from {}", head.status, head.uri);
    queue.follow(format!("?page={}", page + 1)).await?;
    Ok(())
}
```

A URI without a query deserializes from an empty one, so a struct of
optional or defaulted fields always extracts.

## JSON and text

`Json<T>` deserializes the body with serde, which suits APIs. Deriving
`Deserialize` needs serde's `derive` feature in your own dependencies:

```rust
use serde::Deserialize;
use spire::Result;
use spire::extract::Json;
use spire::frontier::queue::Queue;

#[derive(Deserialize)]
struct Page {
    items: Vec<String>,
    next: Option<String>,
}

async fn api(Json(page): Json<Page>, queue: Queue) -> Result<()> {
    println!("{} items", page.items.len());
    if let Some(next) = page.next {
        queue.follow(next).await?;
    }
    Ok(())
}
```

`Text` is the body as a string, and `Bytes` the raw body.

## Writing your own

An extractor implements `FromContext`: it gets the context, with the request
and a way to fetch the response, and the router's state. Its `Rejection` is
what the handler returns instead of running when extraction fails, anything
that converts into an `Outcome`, usually an `Error`:

```rust
use spire::backend::Session;
use spire::extract::FromContext;
use spire::http::http::header::CONTENT_LANGUAGE;
use spire::{Context, Error, Result};

/// The response's `Content-Language`, rejecting pages without one.
struct Language(String);

impl<C: Session, S: Sync> FromContext<C, S> for Language {
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, _: &S) -> Result<Self> {
        let response = cx.response().await?;
        let language = response
            .headers()
            .get(CONTENT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| Error::worker("the page has no language"))?;
        Ok(Self(language.to_owned()))
    }
}

async fn page(Language(language): Language) {
    println!("{language}");
}
```

`cx.response()` fetches the request on its first call and shares the complete
response with every other extractor. `cx.body()` cheaply clones its cached
bytes, including a fixed HTML snapshot for browser responses. An extractor
that only reads the request, like `Uri`, never fetches it.
