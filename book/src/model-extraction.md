# Model extraction

With the `agent` feature, `AgentExtract<T>` converts a page to Markdown and asks
a model for data matching `T`'s JSON Schema. Use it when information is
spread across prose or the page layout varies. CSS selectors remain useful
for predictable markup and need no model calls.

```toml
[dependencies]
spire = { version = "0.8.0", features = ["agent", "reqwest"] }
```

The facade exposes the separate `spire-agent` crate through `spire::agent`.
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
use spire::agent::{Agent, AgentCrawlerExt};
use spire::extract::{AgentExtract, Data};
use spire::{Crawler, Result, Router};
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

async fn products(data: Data<Product>, AgentExtract(products): AgentExtract<Vec<Product>>) -> Result<()> {
    for product in products {
        data.write(product).await?;
    }
    Ok(())
}

# fn router(agent: Agent) {
let router = Router::new().route("products", products);
let crawler = Crawler::new(Noop, router).with_agent(agent);
# let _ = crawler;
# }
```

Register an `InMemDataset<Product>` or another dataset with the crawler
when using this handler. Supply the configured agent with `.with_agent(agent)`;
router state remains available for your own `State<AppState>`. Agent extraction
looks up the request extension before fetching the page and fails if no agent
is registered.

When the handler only stores the extracted records, use `AgentCollect<T>`
instead of writing it yourself:

```rust
use spire::agent::{Agent, AgentCrawlerExt};
use spire::handler::AgentCollect;
use spire::{Crawler, Router};
# use spire::backend::Noop;

# fn router(agent: Agent) {
let router = Router::new().route("names", AgentCollect::<String>::new());
let crawler = Crawler::new(Noop, router).with_agent(agent);
# let _ = crawler;
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

## Next steps

Choose a [model provider](agent-providers.md), configure
[budgets and caches](agent-resources.md), or add
[objective middleware](objectives.md) for relevant discovery and stopping.
