# Crawling toward an objective

An objective describes a goal, which links serve it, and when to stop.
Starting it creates an `ObjectiveRun` that owns one configured agent and
shares progress and relevance decisions across a crawl. These APIs live
in `spire::agent::objective`, behind the `agent` feature.

Use this when the pages to visit depend on what you are trying to learn.
For predictable URL patterns, ordinary [link following](links.md) and
[keyword scores](best-first.md) avoid model calls.

## Configure, start and run

`Objective` is configuration. Its clones are independent, and each
`start(agent)` creates fresh runtime state. `ObjectiveRun` clones share
that state and the model agent selected at startup:

```rust,no_run
use spire::agent::objective::{AgentFrontierExt, Objective};
use spire::agent::{Agent, Budget, Provider};
use spire::backend::ReqwestBackend;
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevant};
use spire::{Crawler, Result, Router};

# async fn crawl(provider: impl Provider) -> Result<()> {
let agent = Agent::new(provider)
    .with_budget(Budget::tokens(100_000).max_cost(2.0))
    .with_concurrency(4);
let objective = Objective::new("find the prices of the available plans")
    .relevant("pricing, plans and billing pages")
    .done_when("the Basic plan has a price")
    .target_items(20)
    .stall_after(50)
    .start(agent);
let router = Router::with_state(objective.clone()).fallback((
    AgentCollect::<String>::new().report_to(&objective),
    FollowRelevant::new("page").same_host(),
));
let crawler = Crawler::new(ReqwestBackend::default(), router)
    .with_frontier(InMemFrontier::priority()
        .dedup()
        .allow_hosts(["example.com"])
        .avoid_traps()
        .until(&objective))
    .with_dataset(InMemDataset::<String>::new())
    .with_seed(Seed::get("https://example.com/")?);
objective.run(&crawler).await?;
println!("{:?}: {:?}", objective.finished(), objective.progress());
# Ok(())
# }
```

Replace `String` with a record such as `Plan` for structured results.
`report_to` additionally requires `Serialize` to retain evidence; ordinary
model extraction only requires `Deserialize + JsonSchema`.

`FollowRelevant` obtains the runtime from router state through `FromRef`.
The facade also selects the runtime's agent for plain extractors and
collectors. Larger state structs can implement those `FromRef` conversions
explicitly.

Keep `.until(&objective)` outside retry, revisit and scope facades so it
observes every popped request and terminal report. Drive the crawler with
`objective.run(&crawler)`: it connects shutdown, drains in-flight work,
and makes a final completion check after exhaustion. The frontier itself
makes no model calls. Calling `crawler.run()` directly with an objective
frontier returns a lifecycle error. Each runtime runs one crawl; start a
fresh runtime to pursue another.

## Rate links together

`FollowRelevant` uses the same visibility checks as `FollowLinks`, including
browser-computed visibility when available. `same_host()` and
`skip_nofollow()` filter links before rating. Ordinary scope, deduplication,
robots.txt and trap limits still apply at the frontier.

A shared rater uses each resolved URL and anchor text to assign a score
from 0 to 100. Scores at or above 50 become link priorities; lower scores
are not followed. Configure the cutoff with `relevance_threshold(score)`
before starting the objective. Use a priority frontier for those scores to
determine crawl order.

By default, up to 256 unique links share a model call. Concurrent pages fill
batches together; partial batches flush after 10ms so concurrency-one
crawls still progress. Configure `rating_batch_size(n)` and
`rating_delay(duration)` on the objective. Handlers await their ratings
and queue writes, so the crawl cannot exhaust while they wait.

Cancelled subscribers are removed before queued batches are dispatched.
Calls already started can finish and are accounted for. Canonical URLs
share decisions, including tracking-parameter variants; the first anchor
seen supplies the text used for a URL. Ratings stay fixed for that run.
The decision map grows with unique candidates, so keep crawl limits in place.

## Settle clear decisions locally

A cheap scorer returns a decisive score or leaves a link uncertain:

```rust
use spire::agent::objective::Objective;
use spire::frontier::score::Keywords;

let config = Objective::new("find plan prices")
    .score_with(Keywords::new(["pricing", "billing"]));
```

Positive keyword matches score 100; nonmatches go to the model.
Negative-weight matches score zero. A closure taking `&Candidate` can
return `Some(score)` for a clear acceptance or rejection, and `None` for
the uncertain middle.

## Report application progress explicitly

Plain `Extract<T>` and `Agent::extract` do not change objective progress.
`AgentCollect<T>::report_to(&objective)` reports the batch only after its
dataset writes succeed. It also checks whether changed evidence satisfies
the completion condition. An extraction or failed dataset write contributes
no reported batch, even if a nontransactional dataset retained partial writes.

Custom handlers have explicit reporting choices:

| Method | Reports |
|---|---|
| `record(item)` | One item with evidence, even when the value is an array |
| `record_batch(items)` | Each supplied item, with evidence |
| `record_page(source, items)` | A successful page's batch, with source attribution |
| `found(n)` | A count without retained evidence |

The application owns uniqueness. Repeated values count again, just as
repeated successful dataset writes store them again. Deduplicate before
reporting when the objective needs unique records. The runtime does not
infer cardinality from JSON shape or hash values to define item identity.
For custom reporting without a collector or `FollowRelevant`, call
`objective.check().await?` when progress changes to request intermediate
completion checks.

The evidence record retains at most 128 items and 32KiB of JSON. Counts
continue beyond those bounds, with omissions in `progress().omitted`.
Completion checks stop once evidence is omitted; use explicit targets for
larger collections. The model sees structured evidence, not an accumulating
free-form conversation.

## Understand why it stopped

| `Finished` | Meaning |
|---|---|
| `Met` | The model judged `done_when` supported by retained evidence |
| `TargetItems` | The explicitly reported item target was reached |
| `TargetPages` | The relevant-page target was reached |
| `Stalled` | Too many pages completed without new relevant pages or items |
| `Exhausted` | The frontier and outstanding requests drained, and a final check did not establish completion |
| `Budget` | A token or USD model budget refused another call |

Relevant pages successfully completed with reported items or a relevant
incoming rating. Terminal failures advance the stall window; skipped and
cancelled pages do not. Targets and stalls require no model call.

Completion judgments require changed progress and run at most once per
five completed pages by default, serialized across handlers. Configure
`check_every(n)` before starting the runtime. Exhaustion makes one final
changed-evidence check, even below that interval. A stale positive judgment
cannot finish a newer progress revision.

The first stop reason wins. Draining work may still report items and usage.
Budget failures return a crawl error as well as preserving `Budget` as the
reason. An unrelated error or external shutdown can leave `finished()` as
`None`. Model judgments and relevance scores are fallible; they do not
prove that a site was fully explored or that its contents are trustworthy.

Run `cargo run -p objective-crawl` for a complete offline example.
