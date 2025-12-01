//! Tower layers example for Spire middleware integration.
//!
//! This example shows minimal setup for using Tower middleware
//! with the Spire web scraping framework.

use spire::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Tower layers example");
    println!("This is a minimal example - Tower middleware integration will be implemented later");

    Ok(())
}
