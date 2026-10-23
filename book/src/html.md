# HTML

The `html` feature, on by default, parses pages with
[scraper](https://docs.rs/scraper). Everything for HTML is in
`spire::extract::html`: the `Html` extractor and its methods, extraction with
`Select`, a page's `Meta` and `JsonLd`, and scraper's own API.

## Parsing pages

`Html` is the parsed page. It finds the title and links, and extracts
anything implementing `Select`:

```rust
use spire::Result;
use spire::extract::html::Html;

async fn page(html: Html) -> Result<()> {
    println!("{:?}", html.title());
    for link in html.links() {
        println!("{link}");
    }
    Ok(())
}
```

`visible_text()` is the text a reader sees: the body without scripts and
styles, one line per paragraph or list item. `main_text()` keeps only the
main content, the first `<main>`, `<article>` or `role="main"` element,
without navigation and asides, for indexing or summarizing pages. To walk
the tree yourself, `Html` wraps a `spire::extract::html::Document`, scraper's
document, and `spire::extract::html` re-exports the rest of scraper at the
version spire uses.

## Selecting with a derive

`#[derive(Select)]`, with the `macros` feature, extracts a struct with CSS
selectors:

```rust
use spire::extract::html::{Element, Select};

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
use spire::extract::html::{JsonLd, Meta};
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

## Walking the tree yourself

`Html` wraps scraper's document, `html::Document`, so a handler can reach
everything scraper offers: elements, nodes and CSS selectors, re-exported in
`spire::extract::html` at the version spire uses, so no dependency of your
own has to match it:

```rust
use spire::extract::html::{Html, Selector};

async fn page(html: Html) {
    let headings = Selector::parse("h2").unwrap();
    for heading in html.0.select(&headings) {
        println!("{}", heading.text().collect::<String>());
    }
}
```
