#[cfg(feature = "client")]
pub use client::HttpClient;
#[cfg(feature = "driver")]
pub use driver::Driver;

use crate::context::{Request, Response};
use crate::BoxError;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "driver")]
pub mod driver;

#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sync + Sized + 'static {
    type Client;
    type Error: Into<BoxError> + Send + Sync + 'static;

    async fn try_resolve(&mut self, request: Request) -> Result<Response, Self::Error>;
}

#[async_trait::async_trait]
impl Backend for () {
    type Client = ();
    type Error = BoxError;

    async fn try_resolve(&mut self, request: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
