//! Basic usage example demonstrating core Spire functionality.
//!
//! This example shows minimal setup and usage of the Spire framework
//! using the reqwest backend for HTTP requests.

use spire::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Basic Spire usage example");
    println!("This is a minimal example - full functionality will be implemented later");

    Ok(())
}
