#[cfg(feature = "client")]
pub use client::Client;
#[cfg(feature = "driver")]
pub use driver::Driver;

use crate::context::{IntoSignal, Request, Response};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "driver")]
mod driver;

#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sync + Sized + 'static {
    type Client;
    type Error: IntoSignal;

    async fn resolve(self, request: Request) -> Result<Response, Self::Error>;
}

#[async_trait::async_trait]
impl Backend for () {
    type Client = ();
    type Error = ();

    async fn resolve(self, request: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
