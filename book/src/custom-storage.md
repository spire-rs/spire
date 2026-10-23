# Custom storage

Every place a crawl keeps state is a trait, so it can live anywhere: a
database, a queue service, or memory shared with something else.

| Trait | Keeps | Built in |
|---|---|---|
| `Frontier` | Seeds waiting to be crawled, and what to do when one ends | `InMemFrontier`, `RedbFrontier` |
| `FrontierLayer` | A facade wrapping a frontier with behavior | `dedup`, `filter`, `max_depth`, `retry`, `tag_backoff`, `revisit` |
| `SeenSet` | Fingerprints of seeds seen before | `InMemSeen`, `BloomSeen`, `RedbSeen` |
| `Schedule` | Seeds held back until a time, for revisits | `InMemFrontier`, `RedbFrontier` |
| `Dataset<T>` | Scraped items | `InMemDataset`, `JsonlDataset`, `CsvDataset`, `RedbDataset` |
| `Validators` | Pages' `ETag` and `Last-Modified` | `InMemValidators`, `RedbValidators` |

## Datasets

A dataset stores items with `write` and hands them back with `read`, first
in, first out, removing them. A write-only sink returns `None`:

```rust
use spire::Result;
use spire::dataset::Dataset;

/// Prints every item instead of storing it.
struct Printed;

impl Dataset<String> for Printed {
    async fn write(&self, item: String) -> Result<()> {
        println!("{item}");
        Ok(())
    }

    async fn read(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
```

`read_batch` and `write_batch` have defaults that go one item at a time;
storage that can do several at once, such as in one transaction, overrides
them, and should store a batch all or nothing.

## Facades

A facade wraps a frontier to change what it accepts or hands out, and a
`FrontierLayer` adds it to a chain with `FrontierExt::layer`:

```rust
use std::future::Future;

use spire::Result;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::frontier::{Ack, Admission, Frontier, FrontierExt, FrontierLayer, Pop, Report};

/// Logs every seed handed out for crawling.
struct Logged<F>(F);

impl<F: Frontier> Frontier for Logged<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.0.push(seed)
    }

    async fn pop(&self) -> Result<Pop> {
        let next = self.0.pop().await?;
        if let Pop::Ready(seed) = &next {
            println!("crawling {}", seed.uri());
        }
        Ok(next)
    }

    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        self.0.report(report)
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        self.0.snapshot()
    }
}

struct Logging;

impl<F: Frontier> FrontierLayer<F> for Logging {
    type Frontier = Logged<F>;

    fn layer(self, inner: F) -> Logged<F> {
        Logged(inner)
    }
}

let frontier = InMemFrontier::fifo().dedup().layer(Logging);
```

A facade passes on what it does not handle: `report`, `size_hint` and
`snapshot` have defaults that refuse retries and hide the queue, so forward
them to the inner frontier, as above.

## Frontiers

A storage frontier implements `push`, `pop` and `report`. `pop` returns
`Pop::Ready` with a seed, `Pop::Wait` until a time when none is ready yet,
or `Pop::Empty`, which ends the crawl once nothing is in flight. `report`
tells it how a popped seed ended: a persistent frontier keeps a seed
recorded until then, so a crash does not lose it. Implementing `Schedule`
as well lets [revisits](recrawls.md) sit on it.

## Seen sets and validators

A `SeenSet` has one method, `insert`, returning whether the fingerprint is
new. `Validators` has `get` and `set`, by the fingerprint of the request.
Both are shared by every request, so they take `&self` and synchronize
inside.
