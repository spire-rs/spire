/// Browser pool builder for configuring and creating pools.
mod builder;
/// Browser connection implementations and pooled connection handling.
mod connection;
/// Internal browser connection manager and lifecycle handling.
mod manager;

pub use builder::BrowserBuilder;
pub use connection::BrowserConnection;
pub use manager::{BrowserPool, WebDriverManager};
