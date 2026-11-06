# spire-agent

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-agent.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-agent
[docs-badge]: https://img.shields.io/docsrs/spire-agent?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-agent

Provider-independent model extraction for Spire.

## Overview

`spire-agent` accepts text directly and depends on `spire-core`, keeping model
providers independent of the facade's router and HTML extractors.

- **`Provider`** makes one completion call, returning an answer and its usage.
  Providers own retries, rate limits and transport timeouts.
- **`objective::Objective<T>`** configures typed goals through worker
  middleware, with independent progress and shared link discovery.
- **`Agent`** shares token usage, budgets and a model-call concurrency limit.
  Structured answers are validated against JSON Schema.

## Installation

Enable the `agent` feature of `spire`, which re-exports this crate as
`spire::agent` and adds `AgentExtract<T>`, `AgentCollect<T>` and
`FollowRelevantLinks` to ordinary crawls:

```toml
[dependencies]
spire = { version = "0.11", features = ["agent", "reqwest"] }
```

Or depend on this crate directly for text extraction without the router or an
HTTP backend:

```toml
[dependencies]
spire-agent = { version = "0.11", features = ["rig"] }
```

No feature is enabled by default:

| Feature | Enables |
|---|---|
| `rig` | Rig completion models as providers |
| `redb` | Answer caching in the crawl's `RedbStore` |
| `sqlite` | Answer caching in the crawl's SQLite store |
| `testing` | Scripted providers for offline tests and examples |

## Usage

Supply a provider and extract typed answers from text. Through `spire`, the
same items are at `spire::agent`, e.g. `use spire::agent::{Agent, Budget};`:

```rust,no_run
use spire_agent::{Agent, Budget, Provider, Result};

# async fn example(provider: impl Provider) -> Result<()> {
let agent = Agent::new(provider)
    .with_budget(Budget::new(100_000, 20_000))
    .with_concurrency(2);
let names: Vec<String> = agent.extract("Our plans are Basic and Pro.").await?;
println!("{names:?}: {:?}", agent.usage());
# Ok(())
# }
```

Budget admission uses settled usage; calls already in flight can cross a
limit. A budget requiring unknown token counts fails closed. Use `Vec<T>` or
`Option<T>` when data may be absent. Schema validation checks structure, not
whether a model's statements are true.

## Offline testing

With `testing`, `Script` records requests and serves fixed responses. Use
`Script::from_fn` for request-dependent replies, or `from_async_fn` to control
completion timing:

```rust
# #[cfg(feature = "testing")] {
use spire_agent::testing::Script;
use spire_agent::{Agent, ProviderResponse, Usage};

# async fn example() -> spire_agent::Result<()> {
let provider = Script::new([ProviderResponse {
    text: r#"{"result":["Basic"]}"#.into(),
    usage: Usage::tokens(10, 5),
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
provider, without a network or API key. `cargo run -p objective-crawl`
demonstrates multiple goals on the same router.

## Learn more

- The book's [Model extraction](https://spire-rs.github.io/spire/model-extraction.html),
  [Model providers](https://spire-rs.github.io/spire/agent-providers.html) and
  [Model budgets and caches](https://spire-rs.github.io/spire/agent-resources.html)
  pages, and [Feature flags](https://spire-rs.github.io/spire/features.html#ai-driven-crawling)
- The [API documentation](https://docs.rs/spire-agent)

## Contributing

See [CONTRIBUTING.md](https://github.com/spire-rs/spire/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

MIT License, see [LICENSE](https://github.com/spire-rs/spire/blob/main/LICENSE).
