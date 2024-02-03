#[cfg(feature = "client")]
pub use client::Client;
#[cfg(feature = "driver")]
pub use driver::Driver;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "driver")]
mod driver;

#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sync + Sized + 'static {}

#[derive(Debug, Clone)]
pub struct TracingBackend {}

impl TracingBackend {}

impl Backend for TracingBackend {}

impl Backend for () {}
