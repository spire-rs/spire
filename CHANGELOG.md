# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Runtime `Seed` extensions, preserved through cloning and retries and moved
  into requests, without affecting serialization or fingerprints.

- `spire-sqlx` with an opt-in `sqlite` backend: caller-owned pools, versioned
  schema migrations and named JSON datasets with transactional FIFO
  reads and writes, persistent seen sets and raw-byte HTTP validators.
  Its frontier uses exclusive sidecar-lock ownership, automatic in-flight
  recovery, persistent priority ordering and scheduled revisits.
  The facade exposes it through `sqlite`, also in `full`, and re-exports
  SQLx as `spire::sqlx`. With `agent`,
  `AgentStoreExt` provides a persistent `SqliteCache` in the same database.

- Offline book link checking in CI and `make book-check`, validating local
  chapter links, heading anchors and assets with pinned Lychee tooling.

## [0.9.0] - 2026-10-29

### Added

- `spire-core`: `Crawler::events`, a stream of typed `crawler::event::Event`s
  for every run: requests starting, fetched (status and latency), handled
  (outcome and elapsed time), retried, failed, skipped or cancelled; hosts
  slowed down or paused; robots.txt fetched; the run starting, stopping and
  finishing. Events are broadcast, so a slow subscriber misses the oldest
  ones, past 1024, and gets `Event::Lagged` instead of slowing the crawl
  down. Without a subscriber, no event is built. Exposed as
  `spire::crawler::event`.
- Objective link ratings share bounded cross-page batches, flushed at 256 unique
  links or after 5 ms by default, independently of handler cancellation.
  Configure these through `Objective::rating_batch_size` and
  `Objective::rating_flush_interval`.

- `spire-agent::objective`: typed `Objective<T>` goals registered on workers
  with `ObjectiveExt::with_objective`. Middleware supplies the shared
  `ObjectiveRegistry`, tracks independent progress and reports each goal's
  `StopReason`. `StopPolicy` selects whether all or any objectives
  must finish; ordinary work exhaustion leaves unmet goals available to
  continue with more seeds.
- `spire`: `FollowRelevantLinks`, rating visible links against active
  objectives and queueing each destination with its highest accepted score.
  `AgentCollect<T>` automatically reports successful writes to the matching
  objective.
- `spire_core::http::UriExt::canonical` (also available through the facade),
  sharing URI normalization across seeds, fingerprints and objective links.
- `spire-core`: `with_extension` on `Crawler`, `Context` and `Visit`.
  `spire::extract::Extension<T>` retrieves crawler or middleware services
  independently of application router state.
- `spire-agent`: `AgentCrawlerExt::with_agent`, configuring model services on
  a crawler without occupying router state.
- `spire-agent`: the `testing` feature and `testing::Script`, a reusable
  provider with queued, request-dependent and asynchronous replies, plus
  request capture. The facade exposes it as `spire::testing::Script` when
  both `agent` and `testing` are enabled.
- `spire`: the `agent-rig` feature, also included in `full`, enables the Rig
  adapter and upstream crate re-export through the facade.
- Conversion from `spire_agent::Error` to `spire_core::Error` (also exposed
  as `spire::Error`), preserving the agent error as its source.
- An offline multi-objective crawl example and an AI-driven crawling book
  section covering extraction, providers, resources and objectives.

### Changed

- `spire-core`: identities are judged by the status the server answered
  with, also when a handler ignores the failed fetch: a swallowed `403`,
  `407` or `429` now counts against the identity, instead of telling
  nothing about it.
- **Breaking:** `spire-agent` budgets now cap input and output tokens
  independently through `Budget::new(input, output)`. Use `input_tokens`
  and `output_tokens` on the default budget to cap only one direction;
  `Budget::tokens` is removed. Unknown usage blocks calls only when that
  direction is capped.
- **Breaking:** renamed `CompletionRequest` and `CompletionResponse` to
  `ProviderRequest` and `ProviderResponse`.
- **Breaking:** moved `spire_agent::Extract<T>` from the agent crate to
  `spire::extract::AgentExtract<T>`. Agent-backed handler extraction now
  obtains its agent through request extensions configured by `with_agent`,
  instead of router state and `FromRef`.
- **Breaking:** renamed `spire::extract::Tab` to `ThirtyfourTab`, identifying
  the backend-specific browser API it exposes.
- **Breaking:** the facade exposes `Data<T>` through `spire::extract::Data`
  instead of `spire::dataset::Data`. Its core location remains
  `spire_core::dataset::Data`.
- **Breaking:** renamed `spire-agent`'s `MemoryCache` to `InMemCache` and the
  core frontier result `Pop` to `NextSeed`; `Frontier::pop()` is unchanged.
- **Breaking:** `spire_agent::rig` now re-exports the upstream Rig crate
  directly, replacing `spire_agent::rig::rig`. Use `spire_agent::Rig` for the
  adapter instead of `spire_agent::rig::Rig`; the facade exposes both through
  its `agent-rig` feature.
- Moved the root README example index to `examples/README.md` and linked
  to the book for feature flags. Shortened rustdoc index summaries and
  expanded their separate descriptions.

### Removed

- **Breaking:** `Pricing`, `Agent::with_pricing` and `Budget::max_cost`.
  Fallback pricing and monetary budget limits are removed.
- **Breaking:** provider cost reporting through `Usage::cost_usd`,
  `Usage::with_cost`, `UsageTotals::cost_usd` and `UsageTotals::unpriced_calls`.
  Usage accounting tracks input/output tokens only.
- **Breaking:** the public `dataset::READ_BATCH` constant in core and the
  facade; `Data::read_all` keeps its internal default batch size.

### Fixed

- `spire::backend` now re-exports `ReqwestSession` alongside `ReqwestBackend`.
- Unknown token usage remains tracked when changing budgets through agent
  clones, preventing later caps from treating unreported usage as zero.

## [0.8.0] - 2026-10-28

### Added

- `spire`: compile-fail tests for the errors of `#[derive(Select)]` and
  `#[derive(Tags)]`, run with trybuild on the CI toolchain.
- `spire-agent`: provider-independent typed model extraction, a Rig adapter,
  shared usage accounting, token and cost budgets, concurrency limits, and
  optional in-memory or redb answer caches. `spire` exposes the crate and
  `Extract<T>` through its `agent` feature.
- `spire`: `AgentCollect<T>`, a handler that stores model-extracted records.
- `spire-core`: `Outcome::Fatal`, stopping a crawl and returning its error.
- `spire-redb`: public transaction callbacks for store extensions. The
  agent cache shares `RedbStore` and its durability policy.
- An offline model extraction example and book chapter.

### Changed

- Reorganized the book by handlers, extraction, discovery, fetching and
  reliability; moved detailed feature flags into a reference page.

## [0.7.0] - 2026-10-27

### Added

- `spire`: `Links::visible`, `PageLink::visibility`, and `Visibility`.
  HTTP clients estimate hidden links from attributes and inline CSS;
  Thirtyfour captures link data and browser-computed visibility together
  through the optional `Session::document` capability and `Document::links`.
- `spire-core`: `FrontierExt::avoid_traps` and `TrapPolicy`, bounding URL
  prefixes by depth, fan-out, candidate count and suspicious path patterns.
  `Admission::PrefixBudgetExceeded` distinguishes budgets from
  `SuspectedTrap`. Refusals are counted and logged.

- `spire`: the `user-agents` feature (included in `full`), backed by uagent
  0.1. `Identity::with_profile` supplies browser navigation headers below
  identity and request overrides; `spire::identity` re-exports `Profile`,
  `UserAgents`, `Browser` and `Platform`. Real browser backends refuse these
  profiles before opening a session.

- `spire`: the `Links` extractor: the links of a page a visitor can follow,
  `<a>` and `<area>`, resolved against its `<base href>` and final URI as a
  browser resolves them, each with its `rel` values, text and kind. Filters
  narrow them (`followable`, `same_host`, `with_rel`, `of_kind`),
  `Links::within` scopes them to a selector, and they convert into links to
  follow.
- `spire`: `FollowLinks::skip_nofollow` leaves out `rel="nofollow"` links.
- `spire`: the `markdown` feature and the `Markdown` extractor, a page's main
  content as Markdown with its links and images resolved against the URI it
  was served from. `Markdown::from_html` converts with `MarkdownOptions`:
  the whole body, without images, or with links as plain text. Conversion is
  done by html-to-markdown-rs.
- `spire-core`: identities. An `Identity` keeps who a request appears to
  come from together: a proxy, the headers that go with it, and its own
  cookies. `Crawler::with_identities` sends every request as one of
  `Identities`, listed with `Identities::new` or built on demand with
  `Identities::from_fn`, up to `max_active` at once, picked in turn, at
  random or per host. Identities whose requests keep failing are retired
  and replaced, or set aside for longer each time. `Seed::with_identity`
  pins a seed to one, `RequestExt::identity` reads it, and robots.txt is
  fetched as one too.
- `spire-core`: `Backend::connect_as` connects a request as its identity,
  through its proxy, keeping each identity's cookies apart. By default it
  refuses an identity with a proxy, so a backend that cannot route through
  one never sends from the crawler's own IP; `MockSite` overrides it, with
  `MockSite::sent` and `unreachable_through` to test crawls with identities.
- `spire-core`: `Identity::downgrade` and `WeakIdentity`, for backends to
  drop what they keep per identity once it is gone.
- `spire-reqwest`: `ReqwestBackend` overrides `connect_as`, with a reqwest client
  per identity, dropped once the identity is gone. `ReqwestBackend::from_builder`
  builds it from a function that makes the reqwest configuration, which
  identities' clients are built from too.
- `spire-thirtyfour`: `ThirtyfourBackend` overrides `connect_as`, with a browser
  profile per identity in every browser it uses: a BiDi user context with its
  proxy, user agent and headers, kept while the identity is in use. Proxies
  asking for credentials are answered with those of the proxy URL.

### Changed

- **Breaking:** `spire-core` exposes `Crawler` through `crawler::Crawler`
  only. The root re-export is removed; `spire::Crawler` is unchanged.

- **Breaking:** `backend::Client` is now `Session`, and `Backend::Client`
  is `Backend::Session`. `Session::fetch` returns a complete
  `Response<Bytes>`; the separate session `body()` operation is removed.
  `Context::session` replaces `client()`, and `Context` caches both the head
  and body. Browser responses are fixed snapshots; live observations use
  the optional `Document` capability or `Tab`. `MockClient` is `MockSession`.

- **Breaking:** requests, responses and their extension traits now live together in
  `spire::http` and `spire_core::http`. The full upstream crate is available
  as `http::http`; `Request`, `Response` and their extension traits are
  re-exported alongside it. `Tag`, `Tags`, `Depth`, `Attempt`, `Revisit` and
  their derive macros live in `crawler`. The old `request` and `response`
  modules and `spire_core`'s direct upstream `http` re-export are removed.

- **Breaking:** `FollowLinks` skips hidden links by default. Unknown links
  remain followable; `include_hidden()` restores following hidden links.

- **Breaking:** renamed `HttpClient` and `HttpSession` to `ReqwestBackend` and
  `ReqwestSession`, and `BrowserBackend`, `BrowserBuilder` and
  `BrowserSession` to `ThirtyfourBackend`, `ThirtyfourBuilder` and
  `ThirtyfourSession`. Backend and session names now identify their
  underlying library.
- **Breaking:** `extract::html` re-exports scraper as `html::scraper`,
  instead of every scraper item, so `html::Selector` is now
  `html::scraper::Selector` and `html::Document` is `html::scraper::Html`,
  like the `reqwest`, `thirtyfour` and `redb` re-exports.
- **Breaking:** `FollowLinks::score_links` scores each link as a `PageLink`,
  with its URI, text and `rel`, instead of its element.
- **Breaking:** `Html::links` takes the page's URI and returns `Links`,
  resolved, instead of the `href`s as written; `Html::anchors` still gives
  them with their elements.
- **Breaking:** `SetHeader::rotate` is removed: rotating a user agent on its
  own is what identities fix. `SetHeader::new` is unchanged.
- **Breaking:** `InMemFrontier` moved from `frontier::memory` to `frontier`,
  as `InMemDataset` and `InMemSeen` are at their modules' roots.
- **Breaking:** `backend::Rotate` is removed: identities cover browser pools
  behind different proxies or with different profiles.
- **Breaking:** `ReqwestBackend` no longer converts `From<reqwest::Client>`: a
  built client cannot be rebuilt for identities. `ReqwestBackend::from_builder`
  takes the function that would build it.
- `spire`: when a page has no `<main>`, `<article>` or `role="main"`
  element, `Html::main_text` leaves the body's `<header>` and `<footer>` out
  too.

### Fixed

- `spire`: `FollowLinks` resolves links against the page's `<base href>`,
  as browsers do; it resolved them against the page's URI alone.

## [0.6.0] - 2026-10-26

Recrawls, with revisits on a schedule and conditional requests, and
best-first crawling, with scores from the URL and from the page. Breaking
changes are marked.

### Added

- `spire-core`: revisits. `FrontierExt::revisit::<T>()` crawls a seed reported
  done or failed again after its tag's `Revisit::interval`, waiting for the
  next one once nothing else is queued. `#[derive(Revisit)]` implements it
  from `#[revisit("1d")]` attributes, checked at compile time. Revisits go
  directly on storage that implements the new `Schedule` trait, one per
  page: `InMemFrontier` in memory, and `RedbFrontier` in its file, so they
  survive a restart.
- `spire-core`: the `Revalidate` middleware sends conditional requests with
  the `ETag` and `Last-Modified` stored in `Validators`, skipping the
  handlers of pages answered `304 Not Modified`, counted in
  `Stats::unchanged`. `InMemValidators` keeps them for one run, and
  `RedbStore::validators` across runs.
- `spire-core`: `MockSite` answers conditional requests with `304`.
- `spire-core`: best-first crawling. `FrontierExt::score` adds a `Score` to
  the priority of every pushed seed, so a priority frontier crawls the best
  first; `Keywords` scores keywords in the URL, `ShallowFirst` shallower
  seeds, and closures from `&Seed` are scorers. Stacked scorers add up, on
  top of the seed's own priority.
- `spire-core`: `Link::with_priority` sets a followed link's priority.
- `spire`: `Html::anchors` returns every link with its `<a>` element.
- `spire`: `FollowLinks::score_page` and `FollowLinks::score_links` give
  followed links a priority: the page's score, computed once, plus each
  link's, e.g. from its anchor text.
- `spire-macros`: `#[derive(FromRef)]` implements `FromRef` for the type of
  every field of a router state, so handlers take parts of it with `State`;
  `#[from_ref(skip)]` leaves a field out.

### Changed

- **Breaking:** scraper, re-exported by `spire::extract::html`, is updated
  from 0.27 to 0.29.

### Fixed

- `spire-thirtyfour`: a `BrowserBackend` dropped without `close()`, or a
  session returning to a closed pool, no longer blocks the runtime while
  thirtyfour tears the sessions down: they are quit on background tasks.
- `spire-redb`: the README's timings now come from the `redb` benchmark.

## [0.5.0] - 2026-10-22

New extractors and one module per format, batched dataset writes, adaptive
host concurrency, and hosts that slow down when they are throttled.
Breaking changes are marked.

### Added

- `spire-core`: throttled hosts slow down below one request at a time. Once
  a host's concurrency cannot go lower, each round of `429`, `503` or
  timeouts doubles the gap between its requests, up to
  the maximum of `with_host_delay`, which now takes a `HostDelay`: a
  `Duration` as its minimum, with a one-minute maximum, or a range.
- `spire-core`: adaptive host concurrency. `with_host_concurrency` takes a
  `HostConcurrency`: a fixed number as before, or
  `HostConcurrency::adaptive(max)`, a limit per host that grows with each
  round of fast responses and halves on `429`, `503`, timeouts, or
  responses slower than twice the host's best.
- `spire`: `Html::visible_text`, the text a reader sees, one line per
  block, and `Html::main_text`, the text of the page's main content without
  navigation.
- `spire`: `spire::extract::html` re-exports scraper, the HTML parser
  `Html` wraps, with its document as `Document`, and `spire::redb::redb`
  the database, so neither needs a matching dependency of its own.
- `spire-core`: `Dataset::write_batch` stores several items at once, by
  default one by one. `InMemDataset`, the file datasets and `RedbDataset`
  store a batch in one step, all of it or none, and `Data::write_all`, which
  `Collect` stores a page's items with, writes one batch.
- `spire`: the `Head` extractor, with the response's status, headers and
  final URI, and `Query<T>`, the request URI's query string deserialized
  with serde.
- `spire`: with the `html` feature, the `Meta` extractor reads a page's
  title, description, canonical URL, language, robots, Open Graph and
  Twitter fields, and `JsonLd<T>` its JSON-LD structured data. `Html::meta`
  and `Html::json_ld` read the same from a parsed page.

### Changed

- `Retry-After` on a `429` or `503` pauses the request's host, not its
  tag, for exactly as long: the host is the one asking. `tag_backoff`
  still applies handler backoffs to tags, and they no longer count as the
  host throttling the crawl.
- **Breaking:** adaptive host delays are replaced by adaptive host
  concurrency, which adapts the one setting a crawl has to choose, how many
  requests a host gets at once, instead of a delay aiming at a separate
  average concurrency. `HostDelay` and `AdaptiveDelay` are removed, and
  `with_host_delay` takes a `Duration` again, as a minimum gap.
- **Breaking:** the redb types moved from `spire::frontier::redb` and
  `spire::dataset` to one `spire::redb` module: `RedbStore`, `RedbFrontier`,
  `RedbSeen`, `RedbDataset` and the `redb` crate.
- **Breaking:** the `Bytes` extractor is renamed `Body`, so it no longer
  clashes with `bytes::Bytes`, which it wraps.
- **Breaking:** everything for HTML is in `spire::extract::html`: `Html`,
  `Element`, `Select` and its derive, `Meta`, `JsonLd`, and scraper's API.
  Everything for sitemaps is in `spire::extract::sitemap`: the `Sitemap`
  extractor and its records, with sitemapo's `Sitemap` record as
  `IndexEntry`. Neither is re-exported at `spire::extract`'s root.
- **Breaking:** `frontier::Next` is renamed `Pop`, the result of
  `Frontier::pop`, apart from the middleware's `Next`.
- **Breaking:** `Scope::Owner` is renamed `Scope::SameTag`.
- **Breaking:** the `View` extractor is renamed `Tab`: it is the request's
  tab in the browser.
- **Breaking:** `spire::extract::sitemapo`, the whole sitemap parser, is
  replaced by the records in `spire::extract::sitemap`.

## [0.4.0] - 2026-10-22

Middleware, adaptive host delays, handler testing and persistent datasets,
with a guide in the new Spire Book. Breaking changes are marked.

### Added

- `spire-redb`: `RedbDataset`, a dataset stored as a table in a `RedbStore`
  next to the frontier, from `RedbStore::dataset(name)`, so one file holds
  the whole crawl. Re-exported as `spire::dataset::RedbDataset`.
- `spire-core`: `CsvDataset`, behind the new `csv` feature, which appends
  every item to a CSV file as a row after a header row. Every row must have
  the columns of the first, and appending checks the file's header.
- `spire-core`: `Dataset::read_batch` takes several items at once, by
  default one by one. `InMemDataset` and `RedbDataset` take a batch in one
  step. `Data::read_all` reads in batches of `READ_BATCH`, and
  `Data::into_batches` streams batches.
- `spire-core`: middleware, re-exported in `spire::worker`. A `Middleware`
  runs around a worker for every request, with the `Next` worker to pass it
  on to: it can change the request, check the response's head before any
  handler, or change the outcome. `WorkerExt::layer` wraps a worker or
  router in it, `middleware::from_fn` makes one from an async function, and
  `SetHeader` sets or rotates a header, e.g. the user agent.
- `spire-core`: adaptive host delays. `Crawler::with_host_delay` takes a
  `HostDelay`: a fixed `Duration` as before, or `HostDelay::adaptive()`,
  which adapts each host's delay to its response times, never lowering it
  on a failure, and doubling it on `429`, `503` or a timeout.
- `spire-core`: `MockPage::with_latency`, for pages that take time to
  answer.
- `spire-core`: `backend::Rotate` cycles through several backends, one per
  request, e.g. HTTP clients behind different proxies.
- [The Spire Book](https://spire-rs.github.io/spire), a guide to every part of
  a crawl, published from `book/`. Its examples are doctested. The `spire`
  README now links to it instead of holding the guide itself.
- `spire-core`: the `testing` module, re-exported as `spire::testing` behind
  the `testing` feature, to test handlers without a network. `MockSite` is a
  backend serving fixed `MockPage`s by URL, with redirects and the real
  backends' status policy, and records what was fetched. `MockSite::visit`
  runs one request through a router or worker, so its outcome, the seeds it
  queued and the items it wrote can be checked without crawling further.

### Changed

- `JsonlDataset` and `CsvDataset` lock their file, so opening one file twice,
  in one process or two, fails instead of mixing writes, and a write that
  fails partway is cut off instead of leaving half an item.
- **Breaking:** `RedbSeen::len`, `RedbSeen::is_empty` and
  `RedbFrontier::in_flight` return a `Result`, so a storage error is no
  longer reported as an empty table, and their `Debug` no longer reads the
  store.
- **Breaking:** `JsonlDataset` needs the new `json` feature. `serde` now only
  makes seeds, fingerprints and Bloom filters serializable, without
  `serde_json` or tokio's file features.
- **Breaking:** `Crawler::new` requires its worker to handle the backend's
  clients. A router's client type is then inferred from the backend where
  the crawler is built, not only once it runs.
- **Breaking:** `spire`'s root holds only `Crawler`, `Router`, `Context`
  and the error types. The crawler's settings and statistics moved to a new
  `spire::crawler` module (`Robots`, `HostDelay`, `AdaptiveDelay`, and
  `Stats`, which was not re-exported before), and the handlers to a public
  `spire::handler` module (`Handler`, `FollowLinks`, `Collect`,
  `FollowSitemap`). The prelude includes `HostDelay`, `FollowLinks` and
  `Collect`.
- **Breaking:** `Crawler::with_host_delay` takes `impl Into<HostDelay>`, so it
  is no longer `const`. A `Duration` still works as before.

## [0.3.0] - 2026-10-21

A redesign around one shared page per request, with sitemaps, built-in
handlers, benchmarks and a faster crawler. Breaking changes are marked.

### Added

- `spire`: the `sitemap` feature. The `Sitemap` extractor parses XML and text
  sitemaps and sitemap indexes, gzipped or not, with `sitemapo`, and the
  `FollowSitemap` handler follows them: pages with one tag, the sitemaps of
  an index with the sitemap's own, queued in the order of their `<priority>`.
- `spire-core`: `Robots::with_sitemaps` queues the sitemaps an origin's
  robots.txt lists, the first time it is fetched.
- `spire`: a tuple of up to 4 handlers is a handler: they run in turn on the
  same page, fetched once, and stop at the first outcome other than
  `Outcome::Continue`.
- Criterion benchmarks of the hot paths, run with `make bench`: the
  crawler's overhead, frontiers, seeds, seen sets, the redb store, routing,
  HTML extraction and sitemap parsing.
- `spire`: the `FollowLinks` handler follows a page's links with a tag,
  optionally only those matching a CSS selector, and `Collect<T>` stores
  every `T` of a page in its dataset (`html` feature).

### Changed

- **Breaking:** handlers and extractors borrow the `Context`, so any
  extractor may read the response and several handlers can share one
  request. `FromContextRef` and `FromContext` are merged into `FromContext`,
  and the "only the last argument fetches" rule is gone.
- **Breaking:** `Client::fetch(&self, &Request)` returns the response's head
  and `Client::body(&self)` its content, and clients are `Sync`; `Context` fetches once, keeps the
  head (`Context::response`) and reads the body from the client
  (`Context::body`), so a browser page reads as currently rendered.
- **Breaking:** `Request` and `Response` carry `Bytes`; the streaming `Body`
  type is removed.
- **Breaking:** `View::html` is `View::snapshot`, to tell the parsed copy
  apart from the browser's own live queries such as `find`.
- **Breaking:** `HttpClient` hands out an `HttpSession` per request.
  `BrowserSession::navigate` and `Navigation` are removed: fetching
  navigates, and `View` reads the status and headers from the head.
- The crawler's per-request overhead is about a third lower: hosts are
  only tracked when a limit, delay or robots.txt needs them, the timers of
  the crawl loop are created once, request futures are boxed before they
  are spawned, crawl metadata is one request extension, and a seed's
  headers and body are boxed, as most seeds have neither.
- **Breaking:** `Seed::headers` and `Seed::body` are no longer `const`.
- **Breaking:** requests get their tag, depth and attempt only from
  `Seed::into_request`; a `Tag` inserted into a request's extensions by hand
  is no longer read.
- Path routes match about 25 times faster: a pattern's literal prefix and
  suffix rule out most paths at once, and matching no longer allocates per
  character.

### Removed

- The `Client` and `Context` extractors, which needed to own the context.

## [0.2.0] - 2026-10-21

The first working release; 0.1.0 only reserved the crate names. Requires
Rust 1.95.

### Added

- `spire-core`: the `Crawler` runs requests concurrently, with per-host
  concurrency and delays, request and crawl budgets
  (`with_max_requests`, `with_time_limit`), per-request timeouts, graceful
  shutdown, and live `Stats`. Handlers steer it with an `Outcome`: continue,
  retry, back off a tag (honouring `Retry-After`), abort a tag, or stop.
- `spire-core`: frontiers queue `Seed`s in FIFO, LIFO or priority order, and
  facades chained with `FrontierExt` add host filters, deduplication over
  normalized URLs, depth limits, retries with jitter, and per-tag backoff.
  Seen sets are exact (`InMemSeen`) or fixed-memory (`BloomSeen`).
- `spire-core`: robots.txt per RFC 9309 (`robots` feature), fetched once per
  origin, honouring `Crawl-delay` and retrying unreachable files.
- `spire-core`: datasets collect scraped items by type (`InMemDataset`, and
  `JsonlDataset` with the `serde` feature, which also makes seeds and
  frontier snapshots serializable). Error statuses follow a shared
  `StatusPolicy`. Every handled request is logged at `debug`.
- `spire`: a `Router` routes by tag, with typed tags from
  `#[derive(Tags)]` checked by `Router::require`, by path pattern for
  untagged requests, or to a fallback. Handlers are async functions of
  extractors: the URI, tag, depth and attempt, the `Queue` for links to
  follow, datasets, shared `State`, and the body as `Bytes`, `Text`, `Json`,
  `Html`, or structs extracted with `#[derive(Select)]` through `Element`.
- `spire-reqwest`: `HttpClient`, an HTTP backend with HTTP/2, system proxies,
  compression (`gzip`, `brotli`, `zstd`, `deflate`), `cookies`, `socks` and
  `hickory-dns` features, and TLS through `rustls-tls` or `native-tls`.
- `spire-thirtyfour`: `BrowserBackend` crawls with real browsers over
  WebDriver BiDi: pooled sessions across remote or `manager`-spawned
  drivers, a fresh browser profile per request, the page's real status and
  headers, and any request method. The `View` extractor hands handlers the
  browser to interact with the page.
- `spire-redb`: `RedbStore` keeps a frontier and seen set in redb, so a crawl
  survives a crash and resumes where it stopped.
- Examples for HTML extraction, JSON APIs and browser automation.

## [0.1.0] - 2025-12-01

### Added

- Initial placeholder release for name reservation

[Unreleased]: https://github.com/spire-rs/spire/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/spire-rs/spire/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/spire-rs/spire/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/spire-rs/spire/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/spire-rs/spire/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/spire-rs/spire/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/spire-rs/spire/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/spire-rs/spire/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/spire-rs/spire/releases/tag/v0.2.0
[0.1.0]: https://crates.io/crates/spire/0.1.0
