//! Types and traits for data retrieval [`Backend`]s.
//!

use tower::Service;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::HttpClient;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::WebDriverPool;

use crate::{BoxError, Error};
use crate::context::{Request, Response};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
mod content;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

// TODO: Replace Backend with Backend2

pub trait Backend2: Service<Request, Response = Response, Error = Error> {
    type Client;
}

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
