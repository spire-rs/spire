//! Reqwest-based HTTP client backend for Spire.
//!
//! This crate provides [`HttpClient`], a Tower service-based HTTP client implementation
//! that integrates with the Spire web scraping framework.
//!
//! # Examples
//!
//! ```ignore
//! use spire_reqwest::HttpClient;
//! use spire_core::Client;
//!
//! let backend = HttpClient::default();
//! let client = Client::new(backend, worker);
//! ```

mod client;

pub use client::HttpClient;
