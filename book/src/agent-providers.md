# Model providers

An `Agent` uses a provider for completion calls. Providers can connect to a
real model or supply deterministic answers for offline tests.

## Provider contract

Implement `Provider::complete` to translate a `ProviderRequest` into one
model call and return a `ProviderResponse`. Forward the system
instructions, input, schema and output limit. Report actual input/output
token counts when available; leave unknown values as `None`. Providers own
retries and rate limits and should report usage for the whole call.

The [model-extraction example](https://github.com/spire-rs/spire/tree/main/examples/model-extraction)
runs a complete crawl with `MockSite` and `spire::testing::Script`, without a
network or API key:

```sh
cargo run -p model-extraction
```

Enable `agent` and `testing` to use `Script` in your own tests. It can
return a queue of `ProviderResponse`s, answer from a synchronous or
asynchronous callback, and capture requests for assertions.

For Rig models, enable `agent-rig` on the facade:

```toml
[dependencies]
spire = { version = "0.9.0", features = ["agent-rig", "reqwest"] }
```

The upstream crate is re-exported as `spire::agent::rig`.

Wrap a Rig completion model with `spire::agent::Rig::new(model)`, then pass
it to `Agent::new`. The adapter includes the schema in the prompt by
default. Opt into `with_native_schema()` when the model supports native
structured output. Answers are validated in both modes. Rig's reported
input/output token usage is preserved.


## Offline tests

Enable `agent` and `testing` on the facade to use the built-in `Script`
provider. Fixed responses are useful for handler tests:

```rust
use spire::testing::Script;
use spire::agent::{Agent, ProviderResponse, Usage};

#[tokio::main(flavor = "current_thread")]
async fn main() -> spire::agent::Result<()> {
    let provider = Script::new([ProviderResponse {
        text: r#"{"result":["Basic"]}"#.into(),
        usage: Usage::tokens(10, 5),
    }]);
    let agent = Agent::new(provider.clone());
    let names: Vec<String> = agent.extract("Basic").await?;
    assert_eq!(names, ["Basic"]);
    assert_eq!(provider.calls(), 1);
    Ok(())
}
```

`Script::from_fn` answers synchronously from each request;
`Script::from_async_fn` supports controlled latency and cancellation.
`requests()` captures calls in arrival order, including failures, while
`calls()` counts them. Clones share the queue and request log. An exhausted
response queue fails instead of silently repeating the last answer.

Use it with [MockSite](testing.md) to exercise the whole crawl without a
network or model API key. Account for [budgets and caches](agent-resources.md)
when asserting the number of provider calls.
