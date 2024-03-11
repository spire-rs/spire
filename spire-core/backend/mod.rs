//! Types and traits for data retrieval [`Backend`]s.
//!

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::HttpClient;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::WebDriverPool;

use crate::context::{Request, Response};
use crate::BoxError;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
mod content;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;
mod exchange;

#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sync + Sized + 'static {
    type Client;
    type Error: Into<BoxError> + Send + Sync + 'static;

    async fn call(&mut self, req: Request) -> Result<Response, Self::Error>;
}

#[async_trait::async_trait]
impl Backend for () {
    type Client = ();
    type Error = BoxError;

    async fn call(&mut self, req: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
