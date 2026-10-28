# Objective middleware

`Objective<T>` describes what accepted records of type `T` should establish.
Wrap a configured router with `.with_objective(...)`; the middleware supplies
its registry automatically. The agent remains independently configured on the
crawler, and the crawler uses its ordinary `run()` method.

```rust
use spire::agent::objective::{Objective, ObjectiveExt};
use spire::agent::{Agent, AgentCrawlerExt, Provider};
use spire::backend::ReqwestBackend;
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevantLinks};
use spire::{Crawler, Result, Router};

# async fn crawl(provider: impl Provider) -> Result<()> {
let router = Router::new()
    .fallback((
        AgentCollect::<String>::new(),
        FollowRelevantLinks::new("page").same_host(),
    ))
    .with_objective(
        Objective::<String>::new("find subscription prices")
            .relevant("pricing, plans and billing")
            .done_when("the Basic plan has a price"),
    );
let prices = router.objective::<String>()?;

let crawler = Crawler::new(ReqwestBackend::default(), router)
    .with_agent(Agent::new(provider))
    .with_frontier(InMemFrontier::priority().dedup().avoid_traps())
    .with_dataset(InMemDataset::<String>::new())
    .with_seed(Seed::get("https://example.com/")?);

crawler.run().await?;
println!("{:?}: {:?}", prices.stop_reason(), prices.progress());
# Ok(())
# }
```

Use a record such as `Plan` instead of `String` for structured answers.
Registering an objective requires `Serialize + JsonSchema`. Ordinary
`AgentCollect<T>` still only requires `Deserialize + JsonSchema`.
`Router::with_state(app_state)` remains available for application dependencies;
objectives and the agent do not occupy router state. Each wrapped request
carries a cheap `ObjectiveRegistry` handle to the middleware's shared services.
The registry stores its data behind an internal `Arc`; looking it up borrows
the handle without copying objective state.

## Multiple goals

Add another `.with_objective(Objective::<Contact>::new(...))` to the same
middleware. Each record type has independent evidence, ratings and completion.
Duplicate record types and missing agents produce configuration errors on the
first request, before fetching its response. Configure routes and other layers
first, then register objectives together on the outer middleware.

`AgentCollect<Plan>` automatically reports successfully stored plans to the
plan objective; `AgentCollect<Contact>` reports contacts. A completed objective's
collector skips further work before fetching. All objectives share the agent's
budget, cache and concurrency gate.

Use one `FollowRelevantLinks` for the group. It rates visible links against active
objectives, takes the highest accepted score for each destination and queues
that URL once per page. Use a priority frontier for best-first order. The
ordinary frontier still owns scope, deduplication, retries and trap limits.
URLs reached from different pages still require frontier deduplication.

A page can serve several objectives through the same handler tuple, sharing
one fetched response. Route overlapping objectives through a common pipeline;
URL deduplication does not dispatch the same request to different router tags.
The [offline example](https://github.com/spire-rs/spire/tree/main/examples/objective-crawl)
collects plans and contacts and follows links with one shared pipeline.

The default stopping policy waits for all objectives. Call
`.stop_when(StopPolicy::Any)` on the middleware to stop when any finishes.
When that condition is reached, middleware returns the existing `Outcome::Stop`.
The crawler stops scheduling and drains requests already in flight.

Finishing one objective does not remove URLs it previously queued. While other
objectives continue, those requests may still be scheduled. Completed collectors
skip their work; a request with no other response-reading handlers may avoid
fetching altogether.

## Progress belongs to the middleware

Obtain a progress handle with `worker.objective::<T>()?` before moving the
worker into the crawler. Handles and worker clones share progress. There is
no separately started runtime and no frontier facade for objectives.

If work runs out, the crawl finishes normally with some goals possibly unmet.
No special final model call or exhaustion state is needed: lack of queued work
adds no evidence. Add more seeds and call `run()` again to continue toward the
same objectives. Create new middleware for fresh progress.

`stop_reason()` returns `Met`, `TargetItems`, `TargetPages` or `Stalled` when a
condition is reached. Otherwise it remains `None`, including after work runs
out or the crawl fails. `target_items` counts accepted records, including
duplicates. `target_pages` counts distinct successful requests with accepted
records or a relevant incoming rating. `stall_after` counts successful requests
without progress; retried and failed requests do not advance that window.

## Evidence and model calls

Collectors serialize a registered objective's evidence before writing and
accept it only after every dataset write succeeds. A nontransactional dataset
can retain partial writes on failure, but that batch contributes no objective
evidence. Plain `AgentExtract<T>` does not imply that application data was accepted.
Custom handlers using `AgentExtract<T>` do not report accepted records automatically.

After a successful worker outcome, the middleware checks changed evidence.
Completion judgments are serialized per objective, and stale positive answers
cannot finish newer progress. A numeric target needs no completion model call.
User stop, retry and error outcomes are preserved rather than replaced by a
completion judgment. Model decisions remain fallible.

Evidence is bounded to 128 records and 32 KiB per objective. Omitted records
still count toward numeric targets, but model judgments stop once evidence is
omitted. This prevents completion claims based on an incomplete evidence set.

Link ratings are cached by canonical URL independently for each objective.
A private shared rater combines unknown links across concurrent pages, flushing
at 256 unique links or after 5 ms by default. Configure these per objective
with `.rating_batch_size(links)` and `.rating_flush_interval(duration)`; zero
delay sends uncached links immediately. Requests for links already being rated
share the result. Cancelling a handler does not cancel its batch; failed ratings are
not cached and can be retried. Keep crawl limits in place because the rating
cache grows with unique URLs. URLs and anchor text are treated as untrusted
data.

Clear decisions can use an optional local scorer:

```rust
use spire::agent::objective::{Candidate, Objective};

let objective = Objective::<String>::new("find prices")
    .score_with(|link: &Candidate| {
        link.url.ends_with("/pricing").then_some(100)
    });
```

`Some(score)` settles a rating locally, clamped to 0..=100; `Some(0)` rejects
the link and `None` asks the model. Each objective applies its own relevance
threshold before scores are merged. The default threshold is 50.
