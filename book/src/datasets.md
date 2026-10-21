# Storing results

Handlers write what they extract to *datasets*, one per item type.
Register a dataset on the crawler, then write to it through the `Data<T>`
extractor:

```rust,no_run
use spire::backend::HttpClient;
use spire::dataset::{Data, InMemDataset};
use spire::extract::Html;
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

`JsonlDataset`, with the `serde` feature, appends every item to a file as
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
the crawl, though not of the machine. A crawl resumed after a crash may
crawl a page again, writing its items twice: deduplicate by a key such as
the URL when reading the file.

## Your own

Implement `spire::dataset::Dataset<T>` to store items anywhere else, such as
a database. It has two methods, `write` and `read`.
