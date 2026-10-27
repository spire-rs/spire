# Identities

A request reveals who appears to send it in several places at once: its IP,
through a proxy; its user agent and the headers that go with it; and its
cookies. Chosen separately, they disagree: one IP sends many user agents,
and a session cookie set through one proxy is sent through another. An
`Identity` keeps them together, one visitor with one proxy, its headers and
its own cookies, and `Crawler::with_identities` sends every request as one.

```rust
use spire::Result;
use spire::identity::{Identities, Identity};
use spire::http::http::HeaderValue;
use spire::http::http::header::{ACCEPT_LANGUAGE, USER_AGENT};

# fn main() -> Result<()> {
let identities = Identities::new([
    Identity::new("de")
        .with_proxy("http://proxy-de.example:8080")?
        .with_header(USER_AGENT, HeaderValue::from_static("MyBot/1.0 (de)"))
        .with_header(ACCEPT_LANGUAGE, HeaderValue::from_static("de-DE,de;q=0.9")),
    Identity::new("us")
        .with_proxy("http://proxy-us.example:8080")?
        .with_header(USER_AGENT, HeaderValue::from_static("MyBot/1.0 (us)")),
])
.sticky_per_host();
# Ok(())
# }
```

An identity's headers are added to every request it sends, unless the
request sets them itself. A proxy URL may carry credentials, as the
gateway below does; they never show up in logs or `Debug` output.

## Browser user agents

Enable `user-agents` (also included in `full`) to give an identity a
[uagent](https://docs.rs/uagent) browser profile: a user agent and matching
navigation headers, including Chromium client hints.

```rust
use spire::identity::{Browser, Identities, Identity, UserAgents};

let agents = UserAgents::new().browsers([Browser::Chrome, Browser::Firefox]);
let identities = Identities::from_fn(move |n| {
    Identity::new(format!("session-{n}"))
        .with_proxy(format!("http://user-session-{n}:secret@gate.example:7000"))
        .map(|identity| identity.with_profile(agents.pick(n)))
});
```

`Profile` and `Platform` are also re-exported from `spire::identity`.
`pick(n)` selects the same profile for the same number, configuration and
uagent version. Keep the lockfile when resuming a crawl: uagent capture
updates can change the selected headers.

Profile headers are defaults: the identity's explicit headers override
them, and the request's headers override both. `Identity::headers()` returns
only explicit headers; `Identity::profile()` returns the separate profile.
Profiles omit `Accept-Encoding`; reqwest advertises the encodings enabled
by its decoders, unless you explicitly override that header.

`ThirtyfourBackend::connect_as` refuses an identity with a user-agent profile
as `Invalid` before opening a session. A real browser supplies its own user
agent and client hints, matching its installed version.

## Many identities

Listing a thousand identities by hand does not scale. `Identities::from_fn`
builds identity `n` on demand, the first time the crawl needs another one,
up to `max_active` at once, so a large pool costs nothing until it is used:

```rust
use spire::Result;
use spire::identity::{Identities, Identity};

# fn main() -> Result<()> {
// A rotating gateway: each session id is a different exit IP.
let gateway = Identities::from_fn(|n| {
    Identity::new(format!("session-{n}"))
        .with_proxy(format!("http://user-session-{n}:secret@gate.example:7000"))
})
.max_active(1000);

// A list of proxies, e.g. read from a file or a provider's API.
let proxies: Vec<String> = vec!["http://10.0.0.1:3128".into(), "http://10.0.0.2:3128".into()];
let count = proxies.len();
let listed = Identities::from_fn(move |n| {
    let proxy = &proxies[usize::try_from(n).unwrap_or_default() % proxies.len()];
    Identity::new(format!("proxy-{n}")).with_proxy(proxy.as_str())
})
.max_active(count);
# Ok(())
# }
```

Names should follow from `n`, as above, so a resumed crawl builds the same
identities again. A function that fails, e.g. on a malformed proxy URL,
fails the request that needed the identity.

## Picking one per request

- `in_turn()`, the default, gives each request the next identity.
- `at_random()` picks one at random.
- `sticky_per_host()` keeps each host on one identity, as one visitor would
  be, for as long as that identity stays in use. A new host gets a new
  identity while fewer than `max_active` are in use.

A seed pinned with `Seed::with_identity(name)` always goes out as that
identity, e.g. a logged-in account. A handler reads the identity its request
went out as with `RequestExt::identity`.

## Failing identities

A request counts against its identity when it gets no answer at all, a
connection failure or a timeout, or an answer that refuses it: `403`, `407`
or `429`. Any other answer resets the count. After `max_failures` in a row,
3 by default, the identity is taken out of use:

- a generated identity is retired, and replaced by the next `n`: a new IP on
  a gateway, the next proxy on a list;
- a listed identity is set aside for `rest`, one minute by default, and
  twice as long each time it fails again.

`retire_after_requests` and `retire_after` retire generated identities
after a number of requests or an age, to spread a crawl over more IPs.
When every identity is set aside, requests are retried once the first
comes back.

robots.txt is fetched as an identity too, the one its host is kept on, so
it does not go out from your own IP. A `Robots` with a backend of its own
fetches without identities.

## Backends

A backend connects each request as its identity, with
`Backend::connect_as`: through the identity's proxy, keeping each
identity's cookies and session apart. A backend that cannot route through a
proxy refuses an identity that has one, failing its requests rather than
sending them from your own IP: that is what `connect_as` does unless a
backend overrides it.

`ReqwestBackend` overrides it with a reqwest client per identity, built from
its configuration with the identity's proxy and, with the `cookies`
feature, a cookie store of its own, dropped once the identity is gone; see
[HTTP](http.md):

```rust,no_run
use std::time::Duration;

use spire::backend::{ReqwestBackend, reqwest};
use spire::identity::{Identities, Identity};
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let backend = ReqwestBackend::from_builder(|| reqwest::Client::builder().timeout(Duration::from_secs(30)))?;
let identities = Identities::from_fn(|n| {
    Identity::new(format!("session-{n}"))
        .with_proxy(format!("http://user-session-{n}:secret@gate.example:7000"))
})
.max_active(100)
.sticky_per_host();

let crawler = Crawler::new(backend, Router::new().fallback(page)).with_identities(identities);
crawler.run().await?;
# Ok(())
# }
```

`ThirtyfourBackend` overrides it with a browser profile per identity, a user
context with its proxy, user agent and headers, see
[Browsers](browsers.md#sessions).

`MockSite` overrides it too, to test crawls with identities:
`MockSite::sent` lists each request with its identity and headers, and
`unreachable_through(name)` makes an identity's proxy refuse every
connection.

## What identities are not

Identities keep a visitor consistent; they are not a disguise. A user agent
names a browser, but a request sent over reqwest's TLS stack does not have
that browser's TLS fingerprint, and sites that check it will see the
difference.

Profiles supply matching header values, but reqwest does not preserve the
browser's header order or emulate its TLS or HTTP/2 fingerprint. See
[issue #60](https://github.com/spire-rs/spire/issues/60). Profiles describe a
direct page navigation; they do not reproduce a browser's changing headers
for every navigation or subresource request.
