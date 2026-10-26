//! `#[derive(FromRef)]` as users write it.

#![cfg(all(feature = "macros", feature = "testing"))]

use std::sync::{Arc, Mutex};

use spire::extract::{FromRef, State};
use spire::frontier::seed::Seed;
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, Router};

/// A part of the state a handler takes on its own.
#[derive(Clone, Default)]
struct Visits(Arc<Mutex<Vec<String>>>);

#[derive(Clone, Default)]
struct Prefix(String);

#[derive(Clone, Default, FromRef)]
struct App {
    visits: Visits,
    prefix: Prefix,
    // Skipped: the state is not reachable as a `String`.
    #[from_ref(skip)]
    #[expect(dead_code, reason = "a skipped field gets no impl")]
    secret: String,
}

async fn page(State(visits): State<Visits>, State(Prefix(prefix)): State<Prefix>) {
    visits.0.lock().unwrap().push(format!("{prefix}page"));
}

#[tokio::test]
async fn handlers_take_parts_of_the_state() {
    let app = App {
        prefix: Prefix("seen ".to_owned()),
        ..App::default()
    };
    let site = MockSite::new().page("https://site.test/", MockPage::html(""));
    let router = Router::with_state(app.clone()).fallback(page);
    let crawler = Crawler::new(site, router).with_seed(Seed::get("https://site.test/").unwrap());

    crawler.run().await.unwrap();
    assert_eq!(*app.visits.0.lock().unwrap(), ["seen page"]);
}

#[test]
fn fields_convert_from_the_state() {
    let app = App {
        prefix: Prefix("p".to_owned()),
        ..App::default()
    };
    let Prefix(prefix) = Prefix::from_ref(&app);
    assert_eq!(prefix, "p");
}
