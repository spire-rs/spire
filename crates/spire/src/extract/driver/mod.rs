//! [`BrowserClient`] extractors.
//!
//! This module provides extractors specifically designed for browser automation
//! using the [`BrowserClient`] backend. These extractors enable interaction with
//! rendered web pages through a WebDriver interface.
//!
//! # Available Extractors
//!
//! - [`View`] - Direct access to the browser's DOM view
//! - `Elements` - Declarative extraction of structured data from rendered pages
//!
//! # Examples
//!
//! ```ignore
//! use spire::extract::driver::View;
//!
//! async fn handler(View(view): View) {
//!     // Access rendered DOM elements
//!     // Note: Current implementation is a placeholder
//! }
//! ```
//!
//! # Note
//!
//! Many extractors in this module are currently placeholders and will be
//! fully implemented in future versions.
//!
//! [`BrowserClient`]: spire_thirtyfour::BrowserClient

mod view;

pub use view::View;
