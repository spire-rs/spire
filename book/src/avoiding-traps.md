# Avoiding traps

A hidden link can lead a crawler into an endless branch of generated
pages. Spire addresses the two parts separately: link filtering skips
hidden entry points, and frontier limits bound URL expansion after a
crawler enters a branch.

Keep obeying [robots.txt](politeness.md#robotstxt): a site may disallow its
trap paths there. Those rules can prevent entry, but do not detect traps
when the site has not declared them.

## Skip hidden links

`FollowLinks` skips hidden links by default, including when narrowed with
`select(...)`. When following links yourself, apply `visible()`:

```rust
use spire::Result;
use spire::extract::html::Links;
use spire::frontier::queue::Queue;

async fn page(links: Links, queue: Queue) -> Result<()> {
    queue.follow_all(links.visible().same_host()).await?;
    Ok(())
}
```

HTTP sessions estimate visibility from attributes and inline CSS. Browser
sessions use computed styles and layout through their live `Document`
capability, so stylesheet rules and script changes are accounted for.
Links below the fold remain followable.

Unknown visibility is kept: an HTTP session cannot establish what a full
browser would render. Raw iteration of `Links` and manual `Queue::follow`
do not apply visibility filtering. `FollowLinks::include_hidden()` opts
out explicitly. See [Hidden links](html.md#hidden-links) for the checks and
their limits, including HTML parsed from a browser snapshot.

## Bound expanding branches

A trap can also have visible links. Add `avoid_traps()` to bound each URL
prefix independently, while allowing the rest of the crawl to continue:

```rust,no_run
use std::sync::Arc;

use spire::backend::ReqwestBackend;
use spire::crawler::Robots;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::FollowLinks;
use spire::{Crawler, Result, Router};

# async fn crawl() -> Result<()> {
let traps = Arc::new(InMemFrontier::fifo()
    .dedup()
    .retry(3)
    .avoid_traps());
let frontier = traps.clone().allow_hosts(["example.com"]);
let crawler = Crawler::new(
    ReqwestBackend::new(),
    Router::new().fallback(FollowLinks::new("page")),
)
.with_frontier(frontier)
.with_robots(Robots::new("MyBot"))
.with_seed(Seed::get("https://example.com/")?);

crawler.run().await?;
println!("{} candidates refused or queued seeds skipped", traps.refused());
# Ok(())
# }
```

The facade checks candidate counts, directory fan-out, path depth,
repeated segments and opaque-looking path segments. Its default scope is
an origin and first path segment: `/archive/` and `/products/` have separate
budgets. Put it outside retry/revisit facades to check their queued work,
and inside scope filters so out-of-scope links are rejected first.

## Tune limits for the site

An archive can be deep and large; product IDs can look random. These
patterns alone do not prove that a branch is a trap. `TrapPolicy` lets you
raise budgets or disable individual checks for a known site. See the
[frontier's trap policy](frontier.md#avoiding-traps) for defaults, prefix
rules and configuration examples.

Budget exhaustion returns `Admission::PrefixBudgetExceeded`; suspicious
path patterns return `Admission::SuspectedTrap`. A blocked prefix refuses
further candidates and skips queued seeds, while in-flight requests may
finish. `refused()` counts both kinds of refusal separately from crawler
request totals. Tracing reports which prefix was blocked and why.

Trap state is in memory and resets when the facade is recreated. These
checks bound crawling; they do not inspect generated text or establish
whether collected content is trustworthy. Keep ordinary request and time
[budgets](politeness.md#budgets) as an overall bound too.
