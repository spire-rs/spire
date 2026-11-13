# HTTP

With the `reqwest` feature, `ReqwestBackend` fetches pages over HTTP with
[reqwest](https://docs.rs/reqwest). Every request of a crawl shares one
connection pool.

```rust,no_run
use std::time::Duration;

use spire::backend::{ReqwestBackend, reqwest};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// reqwest's defaults:
let backend = ReqwestBackend::default();

// Or a configuration of your own, e.g. with a user agent and a timeout.
let backend = ReqwestBackend::from_builder(|| {
    reqwest::Client::builder()
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
  `429` and `5xx` are retried, honouring a valid `Retry-After` on any of
  them; other `4xx` are not. See [Errors and retries](errors.md).
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

Spire follows redirects itself, up to 10 hops. During a crawl, every target
passes frontier scope, robots.txt and the destination host's concurrency and
delay limits before it is fetched. Depth is preserved; `301`, `302` and `303`
apply HTTP method changes, while `307` and `308` retain the method and body.
Cross-origin hops suppress source credentials and select the destination's
applicable cookies. The final URI is recorded for resolving relative links.

The backend disables reqwest's automatic redirect policy and, with `cookies`,
uses one owned jar for direct requests and one for each identity. Builder
redirect policies and cookie providers are replaced. `as_reqwest()` shares
the direct client and its jar, but fetches a single hop. Builder timeouts apply
to each transfer; `Crawler::with_request_timeout` bounds the whole request,
including redirects and admission waits. Other backends must expose hops
through `backend::redirect::Redirects` to support these checks: call `check`
before each transfer, including the initial one, and hold its permit until
the transfer finishes. Adaptive host limits use the destination transfer
time, excluding earlier hops and admission waits.

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
