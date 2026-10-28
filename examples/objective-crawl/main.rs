//! Finds subscription prices and sales contacts through objective middleware.
//! Fixed pages and model responses in `fixtures.rs` keep this example offline.

mod fixtures;

use spire::agent::objective::{Objective, ObjectiveExt};
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::{Deserialize, Serialize};
use spire::agent::{Agent, AgentCrawlerExt, Budget};
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevantLinks};
use spire::{Crawler, Result, Router};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Plan {
    name: String,
    price: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Contact {
    email: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let router = Router::new()
        .fallback((
            AgentCollect::<Plan>::new(),
            AgentCollect::<Contact>::new(),
            FollowRelevantLinks::new("page").same_host(),
        ))
        .with_objective(
            Objective::<Plan>::new("find subscription plans")
                .relevant("pricing and billing")
                .done_when("the Basic plan has a price"),
        )
        .with_objective(Objective::<Contact>::new("find sales contacts").target_items(2));
    let plans = router.objective::<Plan>()?;
    let contacts = router.objective::<Contact>()?;
    let crawler = Crawler::new(fixtures::site(), router)
        .with_agent(Agent::new(fixtures::provider()).with_budget(Budget::new(10_000, 2_000)))
        .with_frontier(InMemFrontier::priority().dedup().allow_hosts(["shop.test"]))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_dataset(InMemDataset::<Contact>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/")?);

    crawler.run().await?;
    for plan in crawler.dataset::<Plan>()?.read_all().await? {
        println!("{}: {}", plan.name, plan.price);
    }
    for contact in crawler.dataset::<Contact>()?.read_all().await? {
        println!("Sales: {}", contact.email);
    }
    println!(
        "Plans: {:?}; contacts: {:?}",
        plans.stop_reason(),
        contacts.stop_reason()
    );
    Ok(())
}
