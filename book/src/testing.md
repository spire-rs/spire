# Testing

With the `testing` feature, `MockSite` serves fixed pages by URL, so
handlers can be tested without a network. Enable it for tests only:

```toml
[dev-dependencies]
spire = { version = "0.3.0", features = ["testing"] }
```

`MockSite` is a backend, so a whole crawl runs against it, and `visit` runs
one request through a router without crawling what it queues, to check what
the handlers did:

```rust
use spire::dataset::InMemDataset;
use spire::extract::Select;
use spire::frontier::seed::Seed;
use spire::testing::{MockPage, MockSite};
use spire::worker::Outcome;
use spire::{Collect, FollowLinks, Result, Router};

#[derive(Debug, Select)]
#[select("article")]
struct Product {
    #[select("h2")]
    name: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let site = MockSite::new().page(
        "https://shop.test/list",
        MockPage::html(r#"<article><h2>Lamp</h2></article><a class="next" href="?page=2">next</a>"#),
    );
    let router = Router::new().route("listing", (
        Collect::<Product>::new(),
        FollowLinks::new("listing").select("a.next"),
    ));

    let visit = site
        .visit(Seed::get("https://shop.test/list")?.with_tag("listing"))
        .with_dataset(InMemDataset::<Product>::new());
    assert!(matches!(visit.run(&router).await, Outcome::Continue));

    let products = visit.dataset::<Product>()?.read_all().await?;
    assert_eq!(products[0].name, "Lamp");
    assert_eq!(visit.queued()[0].uri(), "https://shop.test/list?page=2");
    Ok(())
}
```

Pages are `html`, `text`, `json`, `xml`, a `status`, or a `redirect`, which
is followed as an HTTP client would. URLs without a page get a `404`, and
error statuses fail the fetch as they do over HTTP.
