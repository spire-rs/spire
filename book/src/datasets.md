# Storing results

Handlers write what they extract to *datasets*, one per item type.
Register a dataset on the crawler, then write to it through the `Data<T>`
extractor:

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::dataset::InMemDataset;
use spire::extract::{Data, html::Html};
use spire::frontier::seed::Seed;
use spire::http::http::Uri;
use spire::{Crawler, Result, Router};

#[derive(Debug)]
struct Title {
    uri: String,
    title: String,
}

async fn page(uri: Uri, html: Html, titles: Data<Title>) -> Result<()> {
    let title = html.title().unwrap_or_default();
    titles.write(Title { uri: uri.to_string(), title }).await
}

# async fn crawl() -> Result<()> {
let crawler = Crawler::new(ReqwestBackend::default(), Router::new().fallback(page))
    .with_dataset(InMemDataset::<Title>::new())
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;

for title in crawler.dataset::<Title>()?.read_all().await? {
    println!("{}: {}", title.uri, title.title);
}
# Ok(())
# }
```

A handler taking `Data<T>` for a type without a dataset fails. `Collect<T>`
writes to the dataset for `T` too.

## Choosing storage

| Dataset | Use it for | Reading results |
|---|---|---|
| `InMemDataset` | Temporary results and tests | Take items from memory |
| `JsonlDataset` | Exporting nested or variable-shaped records | Read the JSON Lines file |
| `CsvDataset` | Exporting flat records for spreadsheets | Read the CSV file |
| `RedbDataset` | Keeping results beside a redb frontier | Take persisted JSON records |
| `SqliteDataset` | Keeping results in a caller-owned SQLite database | Take persisted JSON records |

Reads through `Data<T>` remove items from in-memory and database datasets.
JSON Lines and CSV are write-only sinks: their `read` returns `None`, so
consume their files with an external reader.

## In memory

`InMemDataset` keeps items in memory. `new` and `queue` read them in the
order they were written, and `stack` reads the most recent first. Reading
removes them: `read` takes one, and `read_all` takes every item.

## Files

File datasets append serialized records as handlers produce them. Enable
`json` for JSON Lines or `csv` for CSV. Both require `Serialize`.

### JSON Lines

`JsonlDataset`, with the `json` feature, appends every item to a file as
one line of JSON, for items that implement `Serialize`:

```rust,no_run
use serde::Serialize;
use spire::dataset::JsonlDataset;

#[derive(Serialize)]
struct Title {
    uri: String,
    title: String,
}

# async fn example() -> spire::Result<()> {
// `create` truncates the file, and `append` adds to it.
let titles = JsonlDataset::<Title>::create("titles.jsonl").await?;
# Ok(())
# }
```

Each item is written and flushed as it is stored, so it survives a crash of
the crawl, though not of the machine. The file is locked while the dataset
is open, so a second dataset on the same file, in this crawl or another,
fails to open. A crawl resumed after a crash may
crawl a page again, writing its items twice: deduplicate by a key such as
the URL when reading the file.

### CSV

`CsvDataset`, with the `csv` feature, appends every item to a CSV file as
one row, after a header row of its field names:

```rust,no_run
use serde::Serialize;
use spire::dataset::CsvDataset;

#[derive(Serialize)]
struct Product {
    name: String,
    price: Option<f64>,
}

# async fn example() -> spire::Result<()> {
// `create` truncates the file; `append` adds to it, with a header row only
// if it is empty.
let products = CsvDataset::<Product>::create("products.csv").await?;
# Ok(())
# }
```

Items must be flat: fields of strings, numbers, booleans, or `Option`s of
them. An item with a nested struct or a list fails to write. Like JSON
Lines, rows are flushed as they are written, and a resumed crawl may write
a page's rows twice.

## Databases

Database datasets store named collections as JSON and require both
`Serialize` and `Deserialize`. Reopening a store with the same dataset name
makes unconsumed records available to the next run.

### redb

With the `redb` feature, `store.dataset::<T>("products")` keeps items in a
table of the crawl's own database file. The same `RedbStore` can persist its
frontier, seen set, validators and scheduled revisits. See
[Persistence with redb](redb.md) for setup and durability settings.

### SQLite

With the `sqlite` feature, `SqliteStore` uses a caller-owned SQLx pool.
`store.dataset::<T>("products")` selects a named collection in a shared table.
Initialize the store's schema explicitly before using it. See
[Persistence with SQLite](sqlite.md) for pool setup, transactions and frontier recovery.

Dataset writes and frontier updates are separate transactions. A page may
run again after its records were stored, so persistent output can contain
duplicates after recovery. Use an application key such as the URL when
processing results that must be unique.

## Batches

`Data::write_all` writes a collection as one batch, and `Collect<T>` uses it
for the items extracted from one page. File datasets store that batch under
one file lock; database datasets store it in one transaction. A failed batch
write leaves none of its items stored.

`Data::read_all` drains results in batches and collects them into one vector.
For large datasets, `into_batches` yields each batch as a stream, limiting
how much output you hold at once. Database backends can take a batch in one
transaction instead of issuing a transaction per item:

```rust,no_run
use futures::TryStreamExt;
use spire::extract::Data;
# struct Product;

# async fn export(products: Data<Product>) -> spire::Result<()> {
let mut batches = std::pin::pin!(products.into_batches(1000));
while let Some(batch) = batches.try_next().await? {
    println!("{} products", batch.len());
}
# Ok(())
# }
```


## Custom datasets

Implement `spire::dataset::Dataset<T>` for another destination or storage
model. Register it with `with_dataset` and keep using the same handlers and
`Data<T>` extractor. See [Custom storage](custom-storage.md) for the trait and
its batch-operation defaults. Override those methods when the storage can
write or take a whole batch atomically.
