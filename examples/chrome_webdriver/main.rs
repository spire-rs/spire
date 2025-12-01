//! Chrome WebDriver example for Spire browser automation.
//!
//! This example shows minimal setup for browser automation using
//! the thirtyfour backend with Chrome WebDriver.

use spire::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Chrome WebDriver example");
    println!("This is a minimal example - full browser automation will be implemented later");

    Ok(())
}
