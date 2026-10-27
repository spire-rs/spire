//! `#[derive(Tags)]` as users write it.

#![cfg(feature = "macros")]

use spire::backend::Noop;
use spire::crawler::{Tag, Tags};
use spire::frontier::seed::Seed;
use spire::worker::{Outcome, Worker};
use spire::{Context, Router};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Tags)]
enum Page {
    List,
    ItemReviews,
    #[tag(rename = "detail")]
    Item,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Tags)]
#[tag(rename_all = "kebab-case")]
enum Kebab {
    ItemReviews,
    HTTPPage,
}

#[test]
fn names_variants_in_snake_case_by_default() {
    let tags: Vec<Tag> = Page::ALL.iter().map(|&page| page.into()).collect();
    assert_eq!(tags, ["list", "item_reviews", "detail"].map(Tag::from));
}

#[test]
fn renames_variants() {
    assert_eq!(Tag::from(Kebab::ItemReviews).as_str(), "item-reviews");
    assert_eq!(Tag::from(Kebab::HTTPPage).as_str(), "http-page");
}

#[test]
fn parses_tags_back() {
    assert_eq!(Page::from_tag(&Tag::from("detail")), Some(Page::Item));
    assert_eq!(Page::from_tag(&Tag::from("item")), None);
    assert_eq!(
        Kebab::from_tag(&Tag::from("http-page")),
        Some(Kebab::HTTPPage)
    );
}

#[test]
fn routes_by_derived_tags() {
    async fn handler() {}

    let router: Router<Noop> = Router::new()
        .route(Page::List, handler)
        .route(Page::ItemReviews, handler)
        .route(Page::Item, handler)
        .require::<Page>();
    let debug = format!("{router:?}");
    assert!(
        debug.contains(r#"["detail", "item_reviews", "list"]"#),
        "{debug}"
    );
}

fn context(tag: Option<&str>) -> Context<Noop> {
    let mut seed = Seed::get("https://example.com/").unwrap();
    if let Some(tag) = tag {
        seed = seed.with_tag(tag);
    }
    Context::new(seed.into_request(), Noop)
}

#[tokio::test]
async fn extracts_the_variant() {
    async fn page(page: Page) -> Outcome {
        match page {
            Page::List => Outcome::Continue,
            Page::ItemReviews | Page::Item => Outcome::Stop,
        }
    }

    async fn maybe(page: Option<Page>) -> Outcome {
        if page.is_none() {
            Outcome::Stop
        } else {
            Outcome::Continue
        }
    }

    let router: Router<Noop> = Router::new().fallback(page);
    assert!(matches!(
        router.handle(context(Some("list"))).await,
        Outcome::Continue
    ));
    assert!(matches!(
        router.handle(context(Some("detail"))).await,
        Outcome::Stop
    ));

    // Tags of no variant, and untagged requests, are rejected.
    let Outcome::Fail(error) = router.handle(context(Some("other"))).await else {
        panic!("expected a failure");
    };
    assert_eq!(error.to_string(), "tag `other` is not a `Page`");
    assert!(matches!(
        router.handle(context(None)).await,
        Outcome::Fail(_)
    ));

    // `Option` makes it infallible, as for every extractor.
    let router: Router<Noop> = Router::new().fallback(maybe);
    assert!(matches!(router.handle(context(None)).await, Outcome::Stop));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Tags, spire::crawler::Revisit)]
enum Fresh {
    #[revisit("1h30m")]
    Listing,
    #[revisit("1d")]
    Product,
    About,
}

#[test]
fn revisit_intervals_come_from_the_attributes() {
    use std::time::Duration;

    use spire::crawler::Revisit;

    assert_eq!(Fresh::Listing.interval(), Some(Duration::from_mins(90)));
    assert_eq!(Fresh::Product.interval(), Some(Duration::from_hours(24)));
    assert_eq!(Fresh::About.interval(), None);
}
