//! Pursues a pricing objective without a model API key or network access.
//! The scripted provider illustrates the three requests a real provider
//! handles: structured extraction, batched link ratings and completion.

use spire::agent::objective::{AgentFrontierExt, Candidate, Objective};
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::{Deserialize, Serialize};
use spire::agent::serde_json::{Value, json};
use spire::agent::{Agent, Budget, CompletionResponse, Usage};
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevant};
use spire::testing::{MockPage, MockSite, Script};
use spire::{Crawler, Result, Router};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Plan {
    name: String,
    price: String,
}

fn script() -> Script {
    Script::from_fn(|request| {
        // A real provider forwards the request's fields to its model and
        // returns actual usage. These answers are fixed for this local site.
        let text = if request.system.starts_with("Rate each link") {
            let input: Value =
                spire::agent::serde_json::from_str(&request.input).expect("rating input");
            let scores = input["links"]
                .as_array()
                .expect("links")
                .iter()
                .map(|link| {
                    if link["url"].as_str().expect("URL").contains("billing") {
                        80
                    } else {
                        0
                    }
                })
                .collect::<Vec<_>>();
            json!({"scores":scores}).to_string()
        } else if request.system.starts_with("Judge whether") {
            r#"{"met":true}"#.into()
        } else if request.input.contains("Basic: $9") {
            r#"{"result":[{"name":"Basic","price":"$9"}]}"#.into()
        } else {
            r#"{"result":[]}"#.into()
        };
        Ok(CompletionResponse {
            text,
            usage: Usage::tokens(100, 20).with_cost(0.0),
        })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Objective::new("find the prices of our plans")
        .relevant("pricing, plans and billing")
        .done_when("the Basic plan has a price")
        .check_every(1)
        .stall_after(10)
        // Obvious pricing links need no model call. All others remain uncertain.
        .score_with(|link: &Candidate| link.url.ends_with("/pricing").then_some(100));
    let agent = Agent::new(script())
        .with_budget(Budget::tokens(10_000).max_cost(1.0))
        .with_concurrency(2);
    let objective = config.start(agent.clone());
    let site = MockSite::new()
        .page("https://shop.test/", MockPage::html("<main><a href='/billing'>Billing</a><a href='/careers'>Careers</a><a hidden href='/trap'>Hidden pricing</a></main>"))
        .page("https://shop.test/billing", MockPage::html("<main>Basic: $9</main>"));
    let router = Router::with_state(objective.clone()).fallback((
        AgentCollect::<Plan>::new().report_to(&objective),
        FollowRelevant::new("page").same_host(),
    ));
    let crawler = Crawler::new(site, router)
        .with_frontier(
            InMemFrontier::priority()
                .dedup()
                .allow_hosts(["shop.test"])
                .until(&objective),
        )
        .with_dataset(InMemDataset::<Plan>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/")?);
    objective.run(&crawler).await?;
    for plan in crawler.dataset::<Plan>()?.read_all().await? {
        println!("{}: {}", plan.name, plan.price);
    }
    println!("Finished: {:?}", objective.finished());
    println!("Progress: {:?}", objective.progress());
    println!("Usage: {:?}", agent.usage());
    Ok(())
}
