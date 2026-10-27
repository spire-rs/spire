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
use spire::extract::html::{Html, Links};

async fn page(html: Html, links: Links) -> Result<()> {
    println!("{:?}", html.title());
    for link in &links {
        println!("{} {}", link.uri(), link.text());
    }
    Ok(())
}
```

`Links` holds the page's links, resolved as a browser resolves them, with
their `rel` values and text: see [Following links](links.md#the-links-of-a-page).

`anchors()` gives every link together with its `<a>` element, e.g. to read
its text or attributes.

`visible_text()` is the text a reader sees: the body without scripts and
styles, one line per paragraph or list item. `main_text()` keeps only the
main content, the first `<main>`, `<article>` or `role="main"` element,
without navigation and asides, or else the body without its header and
footer as well, for indexing or summarizing pages. To walk
the tree yourself, `Html` wraps scraper's document, and
`spire::extract::html::scraper` re-exports scraper at the version spire
uses.

## Hidden links

`links.visible()` removes links known or estimated to be hidden and keeps
links whose visibility is unknown. `PageLink::visibility()` distinguishes
`Visibility::Visible`, `Hidden` and `Unknown`.

With an HTTP backend, checks use attributes and inline styles on the link
and its ancestors: `hidden`, `display:none`, `visibility:hidden`,
`aria-hidden="true"` paired with `tabindex="-1"`, zero-sized clipped boxes,
and large negative absolute/fixed offsets. A CSS tokenizer handles inline
comments, escapes and declaration boundaries. It does not load external
stylesheets, apply `<style>` rules, execute scripts, or calculate layout.
A link without an obvious hiding rule is `Unknown`, not proven visible.

With `ThirtyfourBackend`, the `Links` extractor and `FollowLinks` use
the `Document` capability returned by `Session::document()`: one capture of
its current URI, base URI, link attributes and visibility. Computed styles
include stylesheets and script changes. Checks cover rendered boxes, opacity, rectangular clipping
and off-canvas placement. Below-the-fold links remain followable. Capture
does not scroll or modify the document. Image-map geometry and unsupported
clip shapes remain `Unknown`; checks do not establish whether another
element covers a link, and do not traverse frames or shadow roots.

`Html::parse`, `Html::links`, `Links::within` and `Tab::snapshot()` operate on
serialized HTML, so their visibility remains an HTML-only estimate even
when that HTML came from a browser. Use the `Links` extractor for rendered
observations. Each extraction observes the current DOM; it is not a live
collection and is not synchronized with a separate `Html` argument.

`FollowLinks` applies `visible()` by default, including with `select(...)`.
Use `include_hidden()` to opt out. Manual `Queue::follow` and raw iteration
of `Links` do not filter visibility automatically. Combine link filtering
with URL expansion limits; see [Avoiding traps](avoiding-traps.md).

## Markdown

With the `markdown` feature, the `Markdown` extractor converts the page's
main content, as `main_text()` finds it, to Markdown: headings, lists,
emphasis, links, images, tables, block quotes and code are kept, which
suits a model's context, a vector store or a search index. Links and images
resolve against the URI the page was served from, and its `<base href>`:

```rust
use spire::extract::html::Markdown;

async fn page(Markdown(markdown): Markdown) {
    println!("{markdown}");
}
```

`Markdown::from_html` converts with options: the whole body instead of the
main content, without images, or with links as plain text:

```rust
use spire::extract::html::{Html, Markdown, MarkdownOptions};
use spire::http::http::Uri;

fn convert(html: &Html, uri: Uri) -> spire::Result<Markdown> {
    let options = MarkdownOptions::new().with_base(uri).whole_body().without_images();
    Markdown::from_html(html, &options)
}
```

The conversion is done by
[html-to-markdown-rs](https://docs.rs/html-to-markdown-rs), which logs an
`html_to_markdown::convert` span for every page.

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

`Html` wraps scraper's document, so a handler can reach everything scraper
offers: elements, nodes and CSS selectors. `spire::extract::html::scraper`
re-exports scraper at the version spire uses, so no dependency of your own
has to match it:

```rust
use spire::extract::html::Html;
use spire::extract::html::scraper::Selector;

async fn page(html: Html) {
    let headings = Selector::parse("h2").unwrap();
    for heading in html.0.select(&headings) {
        println!("{}", heading.text().collect::<String>());
    }
}
```
