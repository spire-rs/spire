//! Fixed pages and model responses for the offline example.

use spire::agent::{CompletionResponse, Usage};
use spire::testing::{MockPage, MockSite, Script};

pub fn script() -> Script {
    Script::new([CompletionResponse {
        text: r#"{"result":[{"name":"Desk lamp","price":"€29"}]}"#.into(),
        usage: Usage::tokens(120, 25).with_cost(0.0),
    }])
}

pub fn site() -> MockSite {
    MockSite::new().page(
        "https://shop.test/products",
        MockPage::html("<h1>Products</h1><p>Desk lamp: €29</p>"),
    )
}
