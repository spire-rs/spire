# spire-agent

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-agent.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-agent
[docs-badge]: https://img.shields.io/docsrs/spire-agent?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-agent
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

Provider-independent model extraction for Spire.

## Overview

`spire-agent` accepts text directly and depends on `spire-core`, keeping model
providers independent of the facade's router and HTML extractors.

- **`Provider`** makes one completion call, returning an answer and its usage.
  Providers own retries, rate limits and transport timeouts.
- **`Agent`** shares token and cost accounting, budgets and a model-call
  concurrency limit. Structured answers are validated against JSON Schema.


The `agent` feature on `spire` re-exports this crate as `spire::agent` and
adds `Extract<T>` and `AgentCollect<T>` to ordinary crawls.

## Usage

For a Spire crawl, enable the facade integration:

```toml
[dependencies]
spire = { version = "0.8.0", features = ["agent", "reqwest"] }
```

For text extraction without the router or an HTTP backend, use this crate
and supply a provider:

```rust,no_run
use spire_agent::{Agent, Budget, Provider, Result};

# async fn example(provider: impl Provider) -> Result<()> {
let agent = Agent::new(provider)
    .with_budget(Budget::tokens(100_000).max_cost(2.0))
    .with_concurrency(2);
let names: Vec<String> = agent.extract("Our plans are Basic and Pro.").await?;
println!("{names:?}: {:?}", agent.usage());
# Ok(())
# }
```

Budget admission uses settled usage; calls already in flight can cross a
limit. A budget requiring unknown tokens or cost fails closed. Use `Vec<T>`
or `Option<T>` when data may be absent. Schema validation checks structure,
not whether a model's statements are true.

### Features

No features are enabled by default:

| Feature | Enables |
|---|---|
| `rig` | The `Rig` adapter for Rig completion models |
| `redb` | `RedbCache` and the `RedbStore` implementation of `AgentStoreExt` |
| `testing` | `testing::Script`, a reusable offline provider |

Enable `rig` directly on this crate when using its adapter. The facade's
`agent + redb` features enable persistent answer caching together, and
`agent + testing` enable the scripted provider.

### Offline testing

With `testing`, `Script` records requests and serves fixed responses. Use
`Script::from_fn` for request-dependent replies, or `from_async_fn` to
control completion timing:

```rust
# #[cfg(feature = "testing")] {
use spire_agent::testing::Script;
use spire_agent::{Agent, CompletionResponse, Usage};

# async fn example() -> spire_agent::Result<()> {
let provider = Script::new([CompletionResponse {
    text: r#"{"result":["Basic"]}"#.into(),
    usage: Usage::tokens(10, 5).with_cost(0.0),
}]);
let agent = Agent::new(provider.clone());
let names: Vec<String> = agent.extract("Basic").await?;
assert_eq!(names, ["Basic"]);
assert_eq!(provider.calls(), 1);
# Ok(())
# }
# }
```

## Examples

Run `cargo run -p model-extraction` for typed extraction with a scripted
provider, without a network or API key.

See the book pages on [Model extraction](https://spire-rs.github.io/spire/model-extraction.html),
[Providers](https://spire-rs.github.io/spire/agent-providers.html),
and [Budgets and caches](https://spire-rs.github.io/spire/agent-resources.html)
in the book, or the [API documentation](https://docs.rs/spire-agent).
