# HTTP

With the `reqwest` feature, `ReqwestBackend` fetches pages over HTTP with
[reqwest](https://docs.rs/reqwest). Every request of a crawl shares one
connection pool.

```rust,no_run
use std::time::Duration;

use spire::backend::ReqwestBackend;
use spire::backend::reqwest::Client;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// reqwest's defaults:
let backend = ReqwestBackend::default();

// Or a configuration of your own, e.g. with a user agent and a timeout.
let backend = ReqwestBackend::from_builder(|| {
    Client::builder()
        .user_agent("MyBot/1.0 (+https://example.com/bot)")
        .timeout(Duration::from_secs(30))
})?;
# Ok(())
# }
```

`from_builder` takes a function that makes the configuration, rather than a
built client, because a crawl with [identities](identities.md) builds a
client per identity from it, dropped once the identity is gone.

`spire::backend::reqwest` re-exports reqwest at the version spire uses.
Compression, cookies, SOCKS proxies and DNS through hickory are features of
`spire-reqwest`, see [Installation](installation.md).

## Error statuses

`ReqwestBackend` applies a `StatusPolicy` to every response, before its body is
downloaded:

- `StatusPolicy::Classify`, the default, turns error statuses into errors,
  so the crawler retries or fails them without a handler checking: `408`,
  `429` and `5xx` are retried, `429` and `503` honouring `Retry-After`, and
  other `4xx` are not. See [Errors and retries](errors.md).
- `StatusPolicy::Accept` hands every response to the handlers, whatever its
  status, e.g. to record `404`s. The `Head` extractor reads the status.

```rust
use spire::backend::{ReqwestBackend, StatusPolicy};

let backend = ReqwestBackend::new().with_status_policy(StatusPolicy::Accept);
```

## Failed transfers

A request that never gets a response fails with an error whose kind says
whether trying again may help:

| Failure | Kind | Retried |
|---|---|---|
| A timeout | `Timeout` | Yes |
| A connection, DNS, protocol or body error | `Http` | Yes |
| A request reqwest cannot build, e.g. an invalid header | `Invalid` | No |
| Too many redirects | `Rejected` | No |

Redirects are followed by reqwest, and the response records the URI it was
served from, which links resolve against: see
[Following links](links.md).

## Proxies

To send requests through proxies, give the crawl
[identities](identities.md): each one a proxy with its own user agent and
cookies. `ReqwestBackend` builds a reqwest client per identity, from its
configuration, with the identity's proxy and, with the `cookies` feature, a
cookie store of its own, and the crawl sets aside proxies that keep
failing. A `socks5` proxy needs the `socks` feature.

Without identities, reqwest takes proxies from the environment by default
(`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`), and a configuration of your own
can route through any proxy reqwest supports, e.g. only some sites through
one with `Proxy::custom`. reqwest picks such a proxy per connection, and
does not notice one that keeps failing.
