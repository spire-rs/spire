use std::fmt;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::BoxError;

pub struct HttpClient {}

impl HttpClient {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        todo!()
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Backend for HttpClient {
    type Client = ();
    type Error = BoxError;

    async fn try_resolve(&mut self, request: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}
