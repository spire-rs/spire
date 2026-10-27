# Routing

A `Router` picks the handler for each request. It is the crawl's worker:

```rust,no_run
# use spire::backend::HttpClient;
# use spire::{Crawler, Router};
# async fn listing() {}
let router = Router::new().route("listing", listing);
let crawler = Crawler::new(HttpClient::default(), router);
```

## Handlers

A handler is an async function whose arguments are extractors, up to 12 of
them, returning anything that converts into an `Outcome`: nothing, a
`Result`, or an `Outcome` itself.

```rust
use spire::Result;
use spire::extract::html::{Html, Links};
use spire::frontier::queue::Queue;
use spire::request::Depth;
use spire::request::http::Uri;

async fn page(uri: Uri, depth: Depth, html: Html, links: Links, queue: Queue) -> Result<()> {
    println!("{uri} at depth {depth}: {:?}", html.title());
    queue.follow_all(links).await?;
    Ok(())
}
```

If an extractor fails, for example because the page is not valid JSON, the
handler does not run and the request fails. [Extractors](extractors.md)
lists every extractor.

## Routes

Routes are tried in this order:

1. **By tag**: `route(tag, handler)` handles the requests with that tag.
2. **By path**: `route_path(pattern, handler)` handles *untagged* requests
   whose path matches the pattern. `*` matches within one path segment and
   `**` across segments; the query is ignored. Path routes are tried in the
   order they were added.
3. **The fallback**: `fallback(handler)` handles the rest.

A request no route takes fails. A tag always wins over a path, and tags are
not inherited: a link queued without a tag is routed by its path.

```rust
use spire::Router;
# use spire::backend::Noop;

async fn listing() {}
async fn product() {}
async fn review() {}
async fn other() {}

let router: Router<Noop> = Router::new()
    .route("listing", listing)
    .route_path("/products/*", product)
    .route_path("/products/*/reviews/**", review)
    .fallback(other);
```

Routing a tag or path twice panics, as does `merge`, which adds another
router's routes, when both routers route the same one.

## Typed tags

`#[derive(Tags)]`, with the `macros` feature, makes an enum of tags:

```rust
use spire::Router;
use spire::request::Tags;
# use spire::backend::Noop;

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    List,
    ItemReviews,
    #[tag(rename = "item")]
    Detail,
}

async fn list() {}
async fn reviews() {}

// A handler serving several variants takes the enum as an argument.
async fn detail(page: Page) {
    println!("{page:?}");
}

let router: Router<Noop> = Router::new()
    .route(Page::List, list)
    .route(Page::ItemReviews, reviews)
    .route(Page::Detail, detail)
    .require::<Page>();
```

- Variants become `snake_case` tags: `ItemReviews` is `item_reviews`.
  `#[tag(rename_all = "kebab-case")]` on the enum picks another case, and
  `#[tag(rename = "...")]` names one variant.
- `require::<Page>()` panics at startup, naming every variant without a
  route, so a forgotten route never surfaces mid-crawl.
- Persistent frontiers store tags as strings. When renaming a variant, keep
  its old name with `#[tag(rename = "old_name")]` so its stored seeds still
  route.

## Several handlers on one page

A tuple of up to 4 handlers runs them in turn on the same page, which is
fetched once. They stop at the first one that does not return
`Outcome::Continue`:

```rust
# use spire::handler::{Collect, FollowLinks};
# use spire::Router;
# use spire::extract::html::Select;
# use spire::backend::Noop;
# #[derive(Debug, Select)]
# #[select("article")]
# struct Product {
#     #[select("h2")]
#     name: String,
# }
let router: Router<Noop> = Router::new().route("listing", (
    Collect::<Product>::new(),
    FollowLinks::new("listing").select("a.next"),
));
```

## Built-in handlers

- **`FollowLinks::new(tag)`** queues every link on the page with `tag`, or
  only those matching `.select(css)`, and without `rel="nofollow"` ones
  with `.skip_nofollow()`.
- **`Collect::<T>::new()`** extracts every `T: Select` on the page and
  writes them to the dataset for `T`.
- **`FollowSitemap::new(tag)`**, with the `sitemap` feature, follows a
  sitemap. See [Sitemaps](sitemaps.md).

## State

`Router::with_state` shares a value with every handler, which takes it, or
a part of it, with the `State` extractor: see [State](state.md).
