//! Fixed pages and model responses for the offline example.

use spire::agent::{ProviderResponse, Usage};
use spire::testing::{MockPage, MockSite, Script};

pub fn script() -> Script {
    Script::new([ProviderResponse {
        text: r#"{"result":[{"name":"Desk lamp","price":"€29"}]}"#.into(),
        usage: Usage::tokens(120, 25),
    }])
}

pub fn site() -> MockSite {
    MockSite::new().page(
        "https://shop.test/products",
        MockPage::html("<h1>Products</h1><p>Desk lamp: €29</p>"),
    )
}
