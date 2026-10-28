//! Extracts typed products from a page without a network or model API key.
//!
//! The scripted provider demonstrates the provider contract; replace it
//! with a real provider to perform model extraction.

mod fixtures;

use spire::agent::cache::InMemCache;
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::Deserialize;
use spire::agent::{Agent, AgentCrawlerExt, Budget};
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::handler::AgentCollect;
use spire::{Crawler, Result, Router};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Product {
    /// The product's displayed name.
    name: String,
    /// The price exactly as displayed, including currency.
    price: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let agent = Agent::new(fixtures::script())
        .with_budget(Budget::tokens(10_000).max_cost(1.0))
        .with_concurrency(1)
        .with_cache(InMemCache::default(), "scripted-products/v1");
    let router = Router::new().route("products", AgentCollect::<Product>::new());
    let crawler = Crawler::new(fixtures::site(), router)
        .with_agent(agent.clone())
        .with_dataset(InMemDataset::<Product>::new())
        .with_seed(Seed::get("https://shop.test/products")?.with_tag("products"));
    crawler.run().await?;

    for product in crawler.dataset::<Product>()?.read_all().await? {
        println!("{}: {}", product.name, product.price);
    }
    println!("Usage: {:?}", agent.usage());
    Ok(())
}
