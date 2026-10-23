# Following links

Pages lead to other pages. A handler queues the links it wants crawled, and
the frontier decides whether to take them: see [The frontier](frontier.md).

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

The `FollowLinks` handler does this for a whole page: every link, or those
matching a CSS selector, with the tag to give them, see
[Routing](routing.md).

## Where links resolve

A relative link resolves against the page it was found on. When the request
was redirected, that is the URI the page was served from: a link `a` on a
page requested as `/old/` and redirected to `/new/` becomes `/new/a`.
`Head::uri`, or `ResponseExt::final_uri` on a response, gives that URI. A
link is one level deeper than its page, which `max_depth` limits.

## Queueing from outside a handler

`Crawler::queue` returns a queue for the crawl's frontier, e.g. to add seeds
while the crawl runs, from a task of your own:

```rust,no_run
use spire::backend::HttpClient;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page));
let queue = crawler.queue();
queue.push(Seed::get("https://example.com/")?).await?;
crawler.run().await?;
# Ok(())
# }
```
