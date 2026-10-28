# Model extraction

With the `agent` feature, `Extract<T>` converts a page to Markdown and asks
a model for data matching `T`'s JSON Schema. Use it when information is
spread across prose or the page layout varies. CSS selectors remain useful
for predictable markup and need no model calls.

```toml
[dependencies]
spire = { version = "0.8.0", features = ["agent", "reqwest"] }
```

The facade re-exports the separate `spire-agent` crate as `spire::agent`.
That crate accepts text directly and does not depend on the facade's HTML
extractors.

## Typed answers

Derive `Deserialize` and `JsonSchema` for the answer type. Field names and
schema descriptions tell the model what to extract. The agent supplies an
object schema wrapping the answer in a `result` field, validates the JSON,
and returns the inner value. Providers must return that complete object.

```rust
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::Deserialize;
use spire::agent::{Agent, Extract};
use spire::dataset::Data;
use spire::{Result, Router};
# use spire::backend::Noop;

#[derive(Deserialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Product {
    /// The product's displayed name.
    name: String,
    /// The price as displayed, including currency.
    price: String,
}

async fn products(data: Data<Product>, Extract(products): Extract<Vec<Product>>) -> Result<()> {
    for product in products {
        data.write(product).await?;
    }
    Ok(())
}

# fn router(agent: Agent) {
let router: Router<Noop, Agent> = Router::with_state(agent).route("products", products);
# let _ = router;
# }
```

Register an `InMemDataset<Product>` or another dataset with the crawler
when using this handler. Put the configured `Agent` in router state;
`FromRef` can select it from a larger application state too.

When the handler only stores the extracted records, use `AgentCollect<T>`
instead of writing it yourself:

```rust
use spire::agent::Agent;
use spire::handler::AgentCollect;
use spire::Router;
# use spire::backend::Noop;

# fn router(agent: Agent) {
let router: Router<Noop, Agent> = Router::with_state(agent)
    .route("names", AgentCollect::<String>::new());
# let _ = router;
# }
```

It extracts `Vec<T>` and writes each item to the registered `T` dataset,
just as `Collect<T>` does for HTML selection. Empty results write nothing;
extraction and budget errors propagate unchanged. Use your own handler
when items need validation or transformation before storing.

Use `Vec<T>` for zero or more records and `Option<T>` when a record may be
absent. Empty arrays and `null` are valid answers for those types. Missing
required data, malformed JSON and schema violations fail extraction.
Schema validation checks structure; it cannot prove that a model's claims
are true. Check important facts against the source.

## Providers

Implement `Provider::complete` to translate a `CompletionRequest` into one
model call and return a `CompletionResponse`. Forward the system
instructions, input, schema and output limit. Report actual input/output
tokens and cost when available; leave unknown values as `None`. Providers
own retries and rate limits and should report usage for the whole call.

The [model-extraction example](https://github.com/spire-rs/spire/tree/main/examples/model-extraction)
runs a complete crawl with `MockSite` and a scripted provider, without a
network or API key:

```sh
cargo run -p model-extraction
```

For Rig models, also enable `rig` on `spire-agent`:

```toml
[dependencies]
spire = { version = "0.8.0", features = ["agent", "reqwest"] }
spire-agent = { version = "0.8.0", features = ["rig"] }
```

Wrap a Rig completion model with `spire::agent::Rig::new(model)`, then pass
it to `Agent::new`. The adapter includes the schema in the prompt by
default. Opt into `with_native_schema()` when the model supports native
structured output. Answers are validated in both modes. Rig's normalized
usage and costs are preserved; configure fallback pricing if needed.

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

`MemoryCache` is a bounded FIFO cache, holding up to 1024 answers by default:

```rust
use spire::agent::Agent;
use spire::agent::cache::MemoryCache;

# fn configure(agent: Agent) -> Agent {
let agent = agent.with_cache(MemoryCache::new(256), "provider/model/settings-v1");
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
