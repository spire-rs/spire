use clap::Parser;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use spire::{Label, Router};

mod handler;
mod service;

/// Command-line arguments.
#[derive(Debug, Parser)]
struct Args {
    /// Bounded server port.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Tracing.
    let env = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "example_browser=trace,tower_http=trace".into());
    let fmt = tracing_subscriber::fmt::layer().pretty();
    tracing_subscriber::registry().with(fmt).with(env).init();

    // Middleware.
    // TODO.

    // Service.
    // TODO: Get rid of explicit State type.
    let state = service::AppState::new();
    let router: Router<service::AppState> = Router::default()
        .route(Label::default(), handler::home_pagination)
        .route(Label::default(), handler::individual_page)
        .with_state(state);

    Ok(())
}
