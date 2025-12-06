//! Browser client implementation with backend/connection architecture.

mod backend;
mod config;

pub use backend::BrowserBackend;
pub use config::{BrowserConfig, BrowserConfigBuilder, PoolConfig, PoolConfigBuilder};
