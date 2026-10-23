# HTTP

With the `reqwest` feature, `HttpClient` fetches pages over HTTP with
[reqwest](https://docs.rs/reqwest). Every request of a crawl shares one
connection pool.

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::backend::reqwest::Client;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// reqwest's defaults:
let backend = HttpClient::default();

// Or a client of your own, e.g. with a user agent and a timeout.
let client = Client::builder()
    .user_agent("MyBot/1.0 (+https://example.com/bot)")
    .timeout(Duration::from_secs(30))
    .build()?;
let backend = HttpClient::from(client);
# Ok(())
# }
```

`spire::backend::reqwest` re-exports reqwest at the version spire uses.
Compression, cookies, SOCKS proxies and DNS through hickory are features of
`spire-reqwest`, see [Installation](installation.md).

## Error statuses

`HttpClient` applies a `StatusPolicy` to every response, before its body is
downloaded:

- `StatusPolicy::Classify`, the default, turns error statuses into errors,
  so the crawler retries or fails them without a handler checking: `408`,
  `429` and `5xx` are retried, `429` and `503` honouring `Retry-After`, and
  other `4xx` are not. See [Errors and retries](errors.md).
- `StatusPolicy::Accept` hands every response to the handlers, whatever its
  status, e.g. to record `404`s. The `Head` extractor reads the status.

```rust
use spire::backend::{HttpClient, StatusPolicy};

let backend = HttpClient::new().with_status_policy(StatusPolicy::Accept);
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

reqwest takes proxies from the environment by default (`HTTP_PROXY`,
`HTTPS_PROXY`, `NO_PROXY`). `Proxy::custom` picks one from the URL whenever
reqwest opens a connection, so one client can keep each host on the same
proxy, or send only some sites through one:

```rust,no_run
use std::hash::{BuildHasher, RandomState};

use spire::backend::HttpClient;
use spire::backend::reqwest::{Client, Proxy};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let proxies = ["http://proxy-a:8080", "http://proxy-b:8080"];
let hasher = RandomState::new();
let client = Client::builder()
    // Each host always goes through the same proxy.
    .proxy(Proxy::custom(move |url| {
        let host = url.host_str()?;
        let index = hasher.hash_one(host) % proxies.len() as u64;
        Some(proxies[index as usize])
    }))
    .build()?;
let backend = HttpClient::from(client);
# Ok(())
# }
```

The choice is made per connection, not per request: reqwest keeps
connections open per host and reuses them, so requests to a host keep
going through the proxy its connection was opened through. Nor does reqwest
notice a proxy that keeps failing; requests through it fail and are retried
like any other.
