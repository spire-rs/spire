# Best-first crawling

A focused crawl reaches the pages most likely to matter first, by giving
every link a priority: a score. It needs a frontier that orders by
priority, `InMemFrontier::priority()` or a redb store's, see
[Seeds and the frontier](frontier.md). Scores come from the URL, in the frontier, and
from the page, in the handler, and add up.

## Scoring by URL

`FrontierExt::score` adds a scorer's score to every seed pushed. A scorer
is a `Score`, or a closure from `&Seed` to `i32`; `Keywords` counts keywords
in the URL's path and query, and `ShallowFirst` scores minus the depth.
Stacked scorers add up:

```rust
use spire::frontier::score::{Keywords, ShallowFirst};
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};

let frontier = InMemFrontier::priority()
    .dedup()
    .score(Keywords::new(["laptop", "review"]).with_weight(10))
    .score(ShallowFirst)
    // Product pages over everything else.
    .score(|seed: &Seed| if seed.uri().path().starts_with("/p/") { 50 } else { 0 });
```

## Scoring by page

A handler has the page a link was found on. `FollowLinks` scores the links
it follows: `score_page` scores the page once, for every link on it, and
`score_links` scores each link from its URI, text and `rel`. The two add
up:

```rust
use spire::frontier::score::Keywords;
use spire::handler::FollowLinks;

let keywords = Keywords::new(["laptop", "review"]);
let page_keywords = keywords.clone();

let follow = FollowLinks::new("page")
    // Links on relevant pages first...
    .score_page(move |page| 10 * page_keywords.matches(&page.title().unwrap_or_default()))
    // ...then by their own text.
    .score_links(move |link| keywords.matches(link.text()));
```

## Scoring in your own handler

When a link's score needs more than the page, such as the router's state or
the request, a handler of your own scores the links: `Html::anchors` gives
every link with its element, and `Link::with_priority` its priority.

```rust
use std::sync::Arc;

use spire::Result;
use spire::extract::State;
use spire::extract::html::{Html, Meta};
use spire::frontier::queue::{Link, Queue};
use spire::frontier::score::Keywords;

struct App {
    keywords: Keywords,
}

async fn page(html: Html, meta: Meta, State(app): State<Arc<App>>, queue: Queue) -> Result<()> {
    // Once per page: how relevant it is.
    let relevance = 10 * app.keywords.matches(&meta.title.unwrap_or_default());
    // Once per link: that, plus its own text.
    let links = html.anchors().map(|(href, anchor)| {
        let text: String = anchor.text().collect();
        Link::from(href).with_priority(relevance + app.keywords.matches(&text))
    });
    let links: Vec<Link> = links.collect();
    queue.follow_all(links).await?;
    Ok(())
}
```

## How scores combine

A link's priority is its base, which the frontier's scorers add to. Seeds
are scored once, when pushed: retries and revisits keep their priority.
Scores only order a frontier that orders by priority; a FIFO or LIFO one
ignores them.

