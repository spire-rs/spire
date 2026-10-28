# How a crawl works

A crawl moves *seeds* from a *frontier*, through a *backend* and a
*worker*, into *datasets*:

```text
            ┌──────────┐  pop   ┌─────────┐ connect ┌─────────┐
  seeds ──▶ │ Frontier │ ─────▶ │ Crawler │ ──────▶ │ Backend │
            └──────────┘        └─────────┘         └─────────┘
                 ▲                   │ Context          │ Session
          follow │                   ▼                  │
                 │      ┌──────────────────────────┐    │
                 └───── │ Worker: Router, handlers │ ◀──┘ fetch
                        └──────────────────────────┘
                              │ write       │ Outcome
                              ▼             ▼
                          Datasets    reported back to the frontier
```

- **A seed** is a request waiting to be crawled: a method and URI, with a
  tag, a depth (how many links away from the first seeds it is), a
  priority, and the attempt it is on. The crawler turns it into a request
  right before fetching.
- **The frontier** holds the seeds. It decides their order, and *facades*
  on top of it add deduplication, depth limits, retries and backoff. See
  [Seeds and the frontier](frontier.md).
- **The crawler** pops seeds and runs up to `with_concurrency` of them at
  once, within the limits of each host. See
  [Limits and politeness](politeness.md).
- **A backend** hands out a session for each request. `ReqwestBackend` fetches
  over HTTP, and `ThirtyfourBackend` loads the page in a browser. See
  [Browsers](browsers.md).
- **The worker** handles each request. It is usually a `Router`, which
  picks handlers by the request's tag. See [Routing and handlers](routing.md).
- **Datasets** store what the handlers extract, one per item type. See
  [Storing results](datasets.md).

## One request, step by step

1. The crawler pops a seed, and waits if its host is busy.
2. The backend hands out a session, and the crawler builds a `Context`:
   the request, the session, a queue for new links, and the datasets.
3. Middleware, if any, runs first. See
   [Middleware](middleware.md).
4. The router picks a handler by the tag, or by the URL's path for an
   untagged request.
5. The handler's extractors read what they need. The first one that needs
   the response fetches it, and the others share it: a page is fetched once
   however many handlers and extractors read it.
6. The handler queues links, writes items, and returns an `Outcome`.
7. The crawler reports the outcome to the frontier: done, retry, pause a
   tag, skip a tag, stop the crawl, or failed.

The crawl ends when the frontier is empty and no request is in flight, when
a handler returns `Outcome::Stop`, when a limit is reached, or when it is
shut down.

Handlers report success, failure or scheduling decisions through an
`Outcome`. See [Errors and retries](errors.md) for their meanings and how
failures affect the crawl.

## `spire` and `spire-core`

The crawler, frontier, backends, workers and datasets live in `spire-core`.
`spire` adds the router, handlers and extractors on top, and re-exports
everything. Depend on `spire-core` directly only to implement your own
backend, frontier or dataset.
