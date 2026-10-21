# Extracting data

Extractors are a handler's arguments. They borrow the request's context, so
any of them may read the response: the first that needs it fetches it, and
the others share it.

| Extractor | Gives |
|---|---|
| `Uri` | The request's URI |
| `Query<T>` | The request URI's query string, deserialized |
| `Tag`, or an enum deriving `Tags` | The request's tag |
| `Depth`, `Attempt` | How many links from a seed it is, and which try this is |
| `Queue` | The queue to follow links through |
| `Data<T>` | The dataset for items of type `T` |
| `State<T>` | The router's state, or a part of it |
| `Head` | The response's status, headers, and final URI after redirects |
| `Bytes` | The response body |
| `Text` | The body as text |
| `Json<T>` | The body deserialized as JSON |
| `Html` | The body parsed as HTML (`html` feature) |
| `Element<T>` | A `T: Select` extracted from the HTML (`html` feature) |
| `Meta` | The page's title, description, canonical URL, language, robots, Open Graph and Twitter fields (`html` feature) |
| `JsonLd<T>` | The page's JSON-LD structured data (`html` feature) |
| `Sitemap` | The body parsed as a sitemap (`sitemap` feature) |
| `Tab` | The browser, to interact with the page (`thirtyfour` feature) |

Wrapping an extractor in `Option` or `Result` makes it infallible: the
handler runs with `None` or the error instead of failing the request.

## HTML

`Html` is the parsed page. It finds the title and links, and extracts
anything implementing `Select`:

```rust
use spire::Result;
use spire::extract::Html;

async fn page(html: Html) -> Result<()> {
    println!("{:?}", html.title());
    for link in html.links() {
        println!("{link}");
    }
    Ok(())
}
```

## Selecting with a derive

`#[derive(Select)]`, with the `macros` feature, extracts a struct with CSS
selectors:

```rust
use spire::extract::{Element, Select};

#[derive(Debug, Select)]
#[select("article.product")]
struct Product {
    #[select("h2")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
    #[select("a.tag")]
    tags: Vec<String>,
    #[select("a.more", attr = "href")]
    link: String,
}

async fn listing(Element(products): Element<Vec<Product>>) {
    for product in products {
        println!("{}: {:?}", product.name, product.price);
    }
}
```

- The `#[select("...")]` on the struct is optional. With it, each element it
  matches is one value; without it, the whole page is one.
- Every field needs a selector: `#[select("css")]` for an element's text,
  `#[select("css", attr = "name")]` for an attribute of it, or
  `#[select(attr = "name")]` for an attribute of the matched element itself.
- The field's type decides how many matches it takes: `Option<T>` the first
  if any, `Vec<T>` all of them, and any other `T` exactly the first, failing
  if there is none. Text is trimmed, with its whitespace collapsed, and
  parsed with `FromStr`.

`Element<T>` takes one value, `Element<Option<T>>` the first if any, and
`Element<Vec<T>>` all of them.

## Metadata and structured data

`Meta` reads a page's `<head>` without selectors: its title, description,
canonical URL, language, `<meta name="robots">`, and Open Graph and Twitter
fields. `JsonLd<T>` reads its [schema.org](https://schema.org) data, which
many product, article and event pages publish in
`<script type="application/ld+json">` blocks, often the most reliable data
on the page:

```rust
use serde::Deserialize;
use spire::Result;
use spire::extract::{JsonLd, Meta};
use spire::frontier::queue::Queue;

/// The nodes to keep, by their `@type`.
#[derive(Deserialize)]
#[serde(tag = "@type")]
enum Node {
    Product { name: String, sku: Option<String> },
    #[serde(other)]
    Other,
}

async fn product(meta: Meta, JsonLd(nodes): JsonLd<Node>, queue: Queue) -> Result<()> {
    // Respect pages asking not to be followed.
    if meta.robots.as_deref().is_some_and(|robots| robots.contains("nofollow")) {
        return Ok(());
    }
    for node in nodes {
        if let Node::Product { name, sku } = node {
            println!("{name} ({sku:?})");
        }
    }
    if let Some(canonical) = meta.canonical {
        queue.follow(canonical).await?;
    }
    Ok(())
}
```

Both parse the page; a handler that also takes `Html` can call
`html.meta()` and `html.json_ld()` instead, to parse it once.

## The response head and query

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

## Following links

`Queue` adds requests to the crawl. Links resolve against the page they were
found on, after redirects, and are one level deeper than it:

```rust
use spire::Result;
use spire::frontier::queue::{Link, Queue};
use spire::frontier::seed::Seed;

async fn page(queue: Queue) -> Result<()> {
    // A relative link, untagged: routed by its path.
    queue.follow("/about").await?;
    // A link with a tag.
    queue.follow(Link::new("/products/1").with_tag("product")).await?;
    // A whole seed, e.g. a POST request.
    let search = Seed::get("https://example.com/search?q=lamp")?.with_tag("search");
    queue.push(search).await?;
    Ok(())
}
```

`follow` and `push` return an `Admission`: whether the request was queued,
or refused as a duplicate, too deep, outside the crawl's scope, or not an
`http(s)` link. Fragments are
dropped, and `follow_all` follows several links at once.
