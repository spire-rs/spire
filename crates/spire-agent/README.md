# spire-agent

Provider-independent extraction from Markdown text, with shared usage,
budgets, concurrency limits and optional answer caching.

No features are enabled by default. `rig` enables the Rig adapter;
`redb` enables a persistent answer cache. The Spire facade's `agent` feature
adds the page extractor and re-exports this crate as `spire::agent`.

See the [model extraction chapter](https://spire-rs.github.io/spire/model-extraction.html)
for typed extraction, provider integration, budgets and caches. Run
`cargo run -p model-extraction` in the repository for a complete scripted
crawl requiring no API key or network.
