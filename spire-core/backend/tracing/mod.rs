use std::future::Ready;
use std::task::{Context, Poll};

use tower::Service;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::Error;

pub struct Trace {}

impl Trace {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Request> for Trace {
    type Response = Response;
    type Error = Error;
    type Future = Ready<Result<Response, Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}

#[async_trait::async_trait]
impl Backend for Trace {
    type Client = ();
}
