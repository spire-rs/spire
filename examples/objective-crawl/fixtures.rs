//! Fixed pages and model responses for the offline example.

use spire::agent::serde_json::{Value, json};
use spire::agent::{ProviderResponse, Usage};
use spire::testing::{MockPage, MockSite, Script};

pub fn provider() -> Script {
    Script::from_fn(|request| {
        let text = if request.system.starts_with("Rate each link") {
            let input: Value = spire::agent::serde_json::from_str(&request.input).unwrap();
            let contacts = input["goal"].as_str().unwrap().contains("contacts");
            let scores = input["links"]
                .as_array()
                .unwrap()
                .iter()
                .map(|link| {
                    let url = link["url"].as_str().unwrap();
                    if url.ends_with("/shared") {
                        if contacts { 95 } else { 60 }
                    } else if contacts && url.ends_with("/contact") {
                        90
                    } else if !contacts && url.ends_with("/pricing") {
                        80
                    } else {
                        0
                    }
                })
                .collect::<Vec<_>>();
            json!({"scores":scores}).to_string()
        } else if request.system.starts_with("Judge whether") {
            r#"{"met":true}"#.into()
        } else if request
            .schema
            .as_ref()
            .unwrap()
            .to_string()
            .contains("email")
        {
            if request.input.contains("Sales contact") {
                if request.input.contains("Enterprise") {
                    r#"{"result":[{"email":"enterprise@shop.test"}]}"#.into()
                } else {
                    r#"{"result":[{"email":"sales@shop.test"}]}"#.into()
                }
            } else {
                r#"{"result":[]}"#.into()
            }
        } else if request.input.contains("Basic: $9") {
            r#"{"result":[{"name":"Basic","price":"$9"}]}"#.into()
        } else {
            r#"{"result":[]}"#.into()
        };
        Ok(ProviderResponse {
            text,
            usage: Usage::tokens(10, 5),
        })
    })
}

pub fn site() -> MockSite {
    MockSite::new()
        .page("https://shop.test/", MockPage::html("<main><a href='/pricing'>Plans</a><a href='/shared'>Overview</a><a href='/contact'>Sales</a><a hidden href='/hidden'>Pricing trap</a></main>"))
        .page("https://shop.test/shared", MockPage::html("<main>Basic: $9. Sales contact. <a href='/contact'>Sales</a></main>"))
        .page("https://shop.test/pricing", MockPage::html("<main>Basic: $9</main>"))
        .page("https://shop.test/contact", MockPage::html("<main>Enterprise Sales contact</main>"))
}
