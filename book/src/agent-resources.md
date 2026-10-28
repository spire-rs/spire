# Model budgets and caches

Model calls share budgets, usage and caches through `Agent` clones.

## Budgets and usage

Configure the agent before registering it with `.with_agent(agent)`. Clones
share usage, concurrency limits and pending identical cache requests.

```rust
use spire::agent::{Agent, Budget};

# fn configure(agent: Agent) -> spire::agent::Result<Agent> {
let agent = agent
    .with_budget(Budget::new(100_000, 20_000))
    .with_concurrency(2)
    .with_max_tokens(2048);
# Ok(agent)
# }
```

`Budget::new(input, output)` caps aggregate input and output tokens separately.
Use `Budget::default().input_tokens(max)` or `.output_tokens(max)` to cap just
one direction. `with_max_tokens` is the per-call output limit, independent of
these aggregate limits.

`agent.usage()` reports known input/output token totals, cache hits and calls
with unknown token counts. Failed or invalid answers still consume reported
usage.

Budgets check settled usage before admitting a call. Already-running calls
can cross either cap; subsequent calls stop once either cap is reached.
Only a capped direction requires known usage for that direction. Missing
output usage does not prevent enforcing an input-only cap, and vice versa.
Cancelled calls leave both token directions unknown, preventing further
budgeted calls. A reached budget stops the crawl and returns an error.
Provider and answer failures fail the page; providers control their retries.

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
