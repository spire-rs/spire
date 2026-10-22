# Your first crawl

This chapter builds a small crawler for an online shop: it walks the
product listing page by page, and extracts every product it links to.

## Tags

Every request carries a *tag* that says what kind of page it is, and the
router picks the handler for it by that tag. Tags can be plain strings, but
an enum deriving `Tags` lets the compiler check them:

```rust
use spire::request::Tags;

/// The kinds of page the crawl visits.
#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Listing,
    Product,
}
```

## What to extract

`#[derive(Select)]` turns CSS selectors into a struct. Here, a product is
the `main.product` element of its page, with a name and an optional price:

```rust
use spire::extract::html::Select;

#[derive(Debug, Select)]
#[select("main.product")]
struct Product {
    #[select("h1")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
}
```

## The crawl

```rust,no_run
use spire::backend::HttpClient;
use spire::dataset::InMemDataset;
use spire::extract::html::Select;
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::Tags;
use spire::handler::{Collect, FollowLinks};
use spire::{Crawler, Result, Router};
#
# #[derive(Debug, Clone, Copy, Tags)]
# enum Page {
#     Listing,
#     Product,
# }
#
# #[derive(Debug, Select)]
# #[select("main.product")]
# struct Product {
#     #[select("h1")]
#     name: String,
#     #[select(".price")]
#     price: Option<f64>,
# }

#[tokio::main]
async fn main() -> Result<()> {
    let router = Router::new()
        // Two handlers on one listing page, fetched once: follow the next
        // page and every product.
        .route(Page::Listing, (
            FollowLinks::new(Page::Listing).select("a.next"),
            FollowLinks::new(Page::Product).select("article.product a"),
        ))
        .route(Page::Product, Collect::<Product>::new())
        .require::<Page>();

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(5).retry(3).tag_backoff())
        .with_dataset(InMemDataset::<Product>::new())
        .with_concurrency(16)
        .with_seed(Seed::get("https://example.com/products")?.with_tag(Page::Listing));

    let stats = crawler.run().await?;
    let products = crawler.dataset::<Product>()?.read_all().await?;
    println!("{} products in {} requests", products.len(), stats.requests);
    Ok(())
}
```

Step by step:

1. **The router.** Listing pages run two built-in handlers in turn:
   `FollowLinks` queues the links matching a selector, with the tag to give
   them. Product pages run `Collect`, which extracts a `Product` and stores
   it. `require` fails at startup if a variant of `Page` has no route.
2. **The frontier** holds the requests waiting to be crawled, in first-in,
   first-out order. On top of it, `dedup` skips URLs already seen,
   `max_depth` stops five links away from the seed, `retry` tries failed
   requests three more times, and `tag_backoff` pauses a tag when a site
   asks to slow down.
3. **The dataset** stores what `Collect` extracts, in memory.
4. **The seed** is the first request: the listing, tagged as one.
5. **`run`** crawls until nothing is queued or in flight, then returns
   statistics. The products are then read back from the dataset.

Relative links resolve against the page they were found on, after
redirects. Error statuses fail the request: `5xx` and `429` are retried,
honouring `Retry-After`, and other `4xx` are not.

The rest of this book goes through each of these pieces in turn.
