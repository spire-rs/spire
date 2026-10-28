# Model budgets and caches

Model calls share resources through `Agent` clones. Objective runs own a
configured agent and use the same accounting for ratings and completion
checks as for extraction through that agent.

## Budgets and usage

Configure the agent before cloning it into state. Clones share usage,
concurrency limits and pending identical cache requests.

```rust
use spire::agent::{Agent, Budget, Pricing};

# fn configure(agent: Agent) -> spire::agent::Result<Agent> {
let agent = agent
    .with_budget(Budget::tokens(100_000).max_cost(2.0))
    .with_pricing(Pricing::new(0.15, 0.60)?)
    .with_concurrency(2)
    .with_max_tokens(2048);
# Ok(agent)
# }
```

Prices are USD per million input and output tokens. Reported provider cost
takes precedence over fallback pricing. `agent.usage()` reports calls,
known tokens and costs, cache hits, and calls with unknown usage or cost.
Failed or invalid answers still consume reported usage.

Budgets check settled usage before admitting a call. Already-running
calls can cross a limit, so these are admission limits rather than a hard
spending guarantee. A reached budget stops the crawl and returns an error.
If usage needed for a budget is missing, further calls stop rather than
assuming they were free. Cancelled calls also leave their usage unknown.
Provider and answer failures fail the page; providers control their own
retries.

## Answer caches

`InMemCache` is a bounded FIFO cache, holding up to 1024 answers by default:

```rust
use spire::agent::Agent;
use spire::agent::cache::InMemCache;

# fn configure(agent: Agent) -> Agent {
let agent = agent.with_cache(InMemCache::new(256), "provider/model/settings-v1");
# agent
# }
```

Choose a namespace identifying the provider, model and relevant settings.
Change it when those change. The key also includes page content,
instructions, schema and output limit, so changed content triggers a new
call. Only validated answers are cached. Cache hits consume no model
budget and remain available after a budget is spent. Identical requests
through clones of one agent share a pending call; separate agents and
processes do not.

For persistence, enable Spire's `agent` and `redb` features and use
`AgentStoreExt::agent_cache()` on your existing `RedbStore`:

```toml
[dependencies]
spire = { version = "0.8.0", features = ["agent", "redb"] }
```

```rust,no_run
use spire::agent::{Agent, Provider};
use spire::agent::cache::AgentStoreExt;
use spire::redb::RedbStore;

# async fn configure(provider: impl Provider) -> spire::Result<Agent> {
let store = RedbStore::open("crawl.redb").await?;
let cache = store.agent_cache();
let agent = Agent::new(provider).with_cache(cache, "provider/model/settings-v1");
# Ok(agent)
# }
```

The cache shares the store's database and durability policy. It creates a
namespaced table lazily, so the crawl's frontier and cached answers can be
reopened together.
