# Storing results

Handlers write what they extract to *datasets*, one per item type.
Register a dataset on the crawler, then write to it through the `Data<T>`
extractor:

```rust,no_run
use spire::backend::ReqwestBackend;
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

With the `redb` feature, `store.dataset::<T>(name)` keeps items of type `T`
in a table of the crawl's own redb file, as JSON, for types that implement
both `Serialize` and `Deserialize`. Items survive a crash with the rest of
the crawl: see [Crash safety with redb](redb.md).

## Reading in batches

`Data::read_all` takes every item, a batch at a time, and `into_batches`
streams them, which suits large datasets. Datasets that can take several
items at once do: a redb dataset takes a batch in one transaction instead of
one per item. Writing works the same way: `Collect` writes a page's items as
one batch, which the file and redb datasets store all at once or not at all.

```rust,no_run
use futures::TryStreamExt;
use spire::dataset::Data;
# struct Product;

# async fn export(products: Data<Product>) -> spire::Result<()> {
let mut batches = std::pin::pin!(products.into_batches(1000));
while let Some(batch) = batches.try_next().await? {
    println!("{} products", batch.len());
}
# Ok(())
# }
```

## Your own

Implement `spire::dataset::Dataset<T>` to store items anywhere else, such as
a database: see [Custom storage](custom-storage.md).
