//! [`BrowserConnection`] extractors.
//!
//! This module provides extractors specifically designed for browser automation
//! using the [`BrowserConnection`] backend. These extractors enable interaction with
//! rendered web pages through a `WebDriver` interface.
//!
//! # Available Extractors
//!
//! - [`View`] - Direct access to the browser's DOM view
//!
//! # Examples
//!
//! ```ignore
//! use spire::extract::driver::View;
//!
//! async fn handler(View(driver): View) {
//!     let title = driver.title().await;
//! }
//! ```
//!
//! [`BrowserConnection`]: spire_thirtyfour::BrowserConnection

mod view;

pub use view::View;
