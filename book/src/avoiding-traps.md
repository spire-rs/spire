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
raise budgets or disable individual checks for a known site:

```rust
use std::sync::Arc;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::frontier::facade::TrapPolicy;

let policy = TrapPolicy::default()
    .with_max_urls(10_000)
    .with_max_fanout(1_000)
    .with_max_random_urls(usize::MAX); // This site's paths contain opaque IDs.
let traps = Arc::new(InMemFrontier::fifo()
    .dedup()
    .retry(3)
    .avoid_traps_with(policy));
let frontier = traps.clone().allow_hosts(["example.com"]);
// Keep the inner handle to inspect refusals after handing frontier to the crawler.
assert_eq!(traps.refused(), 0);
```

A prefix is an origin and first nonempty path segment:
`https://example.com/archive/` is independent of `/products/`, other
hosts, ports and schemes. Opaque first segments share the origin's root
prefix so changing that segment cannot bypass the opaque-URL budget.
Defaults are:

| Check | Limit | Refusal |
|---|---:|---|
| Distinct request fingerprints offered per prefix | 4,096 | `PrefixBudgetExceeded` |
| Candidates under the same directory | 256 | `PrefixBudgetExceeded` |
| Nonempty segments in a URL path | 32 | `PrefixBudgetExceeded` |
| Occurrences of the same segment in one path | 4 | `SuspectedTrap` |
| URLs with opaque-looking segments per prefix | 8 | `SuspectedTrap` |

Opaque-looking means at least 16 ASCII alphanumeric characters, mixing
letters and digits, with at least eight distinct characters. Real IDs can
match. Adjust `TrapPolicy` for the site's structure; use `usize::MAX` to
disable a check. Date archives and ordinary pagination are not opaque, but
can still reach explicit budgets. Fan-out is estimated from URL
directories, not the number of links found on one page. Different query
variants share the same directory budget. These are candidate budgets:
inner facades may subsequently refuse a candidate, and it still counts;
offering the same request fingerprint again does not count twice.

Crossing a limit blocks that prefix for the facade's lifetime. Further
pushes return its refusal reason. Already-queued seeds from it are skipped
when popped and reported inwards as `Report::Skipped`; in-flight requests
can finish. Other prefixes continue. `AvoidTraps::refused()` counts refused
pushes and skipped queued seeds, separately from crawler request totals.
Tracing logs identify the blocked prefix and the reason.

Place this facade outside retry/revisit facades to check what they pop,
and inside scope filters to avoid tracking out-of-scope candidates. Keep
`dedup()` too: the facade's observations do not replace deduplication.
Counters and blocked-prefix state are in memory and reset when the facade
is recreated. Snapshots omit known blocked seeds but do not persist the
heuristic state; pending seeds in persistent storage are skipped when
popped after their prefix becomes blocked.

These checks bound crawling; they do not inspect generated text or
establish whether collected content is trustworthy. Keep ordinary request
and time [budgets](politeness.md#budgets) as an overall bound too.
