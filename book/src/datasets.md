# Storing results

Handlers write what they extract to *datasets*, one per item type.
Register a dataset on the crawler, then write to it through the `Data<T>`
extractor:

```rust,no_run
use spire::backend::HttpClient;
use spire::dataset::{Data, InMemDataset};
use spire::extract::html::Html;
use spire::frontier::seed::Seed;
use spire::request::http::Uri;
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
let crawler = Crawler::new(HttpClient::default(), Router::new().fallback(page))
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

## In memory

`InMemDataset` keeps items in memory. `new` and `queue` read them in the
order they were written, and `stack` reads the most recent first. Reading
removes them: `read` takes one, and `read_all` takes every item.

## JSON Lines

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

## CSV

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

## In the crawl's database

With the `redb` feature, a `RedbStore` holds datasets next to the frontier
and the seen set, so one file holds the whole crawl. Items are stored as
JSON, for types that implement both `Serialize` and `Deserialize`, and read
back in the order they were written:

```rust,no_run
use serde::{Deserialize, Serialize};
use spire::backend::HttpClient;
use spire::frontier::FrontierExt;
use spire::redb::RedbStore;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};

#[derive(Serialize, Deserialize)]
struct Product {
    name: String,
    price: Option<f64>,
}

# async fn crawl(router: Router<<HttpClient as spire::backend::Backend>::Client>) -> Result<()> {
let store = RedbStore::open("crawl.redb").await?;
let crawler = Crawler::new(HttpClient::default(), router)
    .with_frontier(store.frontier().dedup_with(store.seen()))
    // Each dataset is a table, by name.
    .with_dataset(store.dataset::<Product>("products"))
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;

let products = crawler.dataset::<Product>()?.read_all().await?;
# Ok(())
# }
```

Items are committed as they are written, and synced to disk with the
frontier, so after a crash the reopened store has every item written before
it. As with files, a page whose items were committed just before the crash
may be crawled again and write them twice. Reading takes items out of the
table: read them once the crawl is over.

## Your own

Implement `spire::dataset::Dataset<T>` to store items anywhere else, such as
a database. It has two methods, `write` and `read`.
