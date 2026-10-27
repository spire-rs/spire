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

The `FollowLinks` handler does this for a whole page: visible or unknown
links, or those matching a CSS selector, with the tag to give them, and without
`rel="nofollow"` ones with `skip_nofollow`, see [Routing](routing.md).

`Link::with_priority` sets how soon a link is crawled, on a frontier that
orders by priority, and `FollowLinks` scores the links it follows with
`score_page` and `score_links`:
see [Best-first crawling](best-first.md).

## The links of a page

The `Links` extractor, with the `html` feature, gives the links a visitor
can follow, `<a>` and `<area>`, already resolved, each with its `rel` values
and the text it shows. Filters narrow them, and they queue as they are:

```rust
use spire::Result;
use spire::extract::html::Links;
use spire::frontier::queue::{Link, Queue};

async fn page(links: Links, queue: Queue) -> Result<()> {
    for link in &links {
        println!("{} {:?} {}", link.uri(), link.rel(), link.text());
    }
    // The next page of a listing, on this site, if the site marks it.
    let next = links.clone().followable().same_host().with_rel("next");
    queue.follow_all(next).await?;
    // Every other link, tagged by its text.
    let products = links
        .into_iter()
        .filter(|link| link.text().contains("lamp"))
        .map(|link| Link::from(link).with_tag("product"));
    queue.follow_all(products).await?;
    Ok(())
}
```

- `visible()` leaves out hidden links and keeps unknown ones; see
  [Hidden links](html.md#hidden-links) for HTTP and browser checks.
- `followable()` leaves out `rel="nofollow"` links, `same_host()` links to
  other hosts, subdomains included, `with_rel(value)` keeps those with a
  `rel` value, and `of_kind` those from `<a>` or `<area>`.
- `Links::within(&html, &uri, "article")` reads only the links inside
  matching elements, e.g. an article without its navigation.
- `Html::links(&uri)` does the same for a page parsed by hand, e.g. a
  browser tab's `snapshot` after interacting with it.
- Links that are not `http(s)` URLs, such as `mailto:` or `javascript:`, are
  left out, and fragments are dropped.

For hidden entry points and expanding URL branches, see
[Avoiding traps](avoiding-traps.md).

## Where links resolve

A relative link resolves against the page it was found on, as a browser
resolves it: against the page's `<base href>`, if it has one, itself
resolved against the URI the page was served from. When the request was
redirected, that is the final URI: a link `a` on a page requested as `/old/`
and redirected to `/new/` becomes `/new/a`. `Head::uri`, or
`ResponseExt::final_uri` on a response, gives that URI. `Links` and
`FollowLinks` resolve this way; a relative `href` passed to `Queue::follow`
resolves against the final URI alone. A link is one level deeper than its
page, which `max_depth` limits.

## Queueing from outside a handler

`Crawler::queue` returns a queue for the crawl's frontier, e.g. to add seeds
while the crawl runs, from a task of your own:

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let crawler = Crawler::new(ReqwestBackend::default(), Router::new().fallback(page));
let queue = crawler.queue();
queue.push(Seed::get("https://example.com/")?).await?;
crawler.run().await?;
# Ok(())
# }
```
