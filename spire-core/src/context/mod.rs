//! Request context and extensions for web scraping.
//!
//! This module provides the [`Context`] type that wraps HTTP requests with additional
//! metadata and utilities for web scraping, including:
//!
//! - Request/response body handling
//! - Request queuing and routing via tags
//! - Signal-based flow control
//! - Dataset access for storing scraped data

mod body;
mod context;
mod extend;
mod queue;
mod signal;

pub use body::{Body, Request, Response};
pub use context::Context;
pub use extend::{Depth, Tag, Task, TaskBuilder};
pub use queue::RequestQueue;
pub use signal::{IntoSignal, Signal, TagQuery};
