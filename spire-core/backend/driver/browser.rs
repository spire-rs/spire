use std::fmt;
use std::ops::{Deref, DerefMut};
use std::task::{Context, Poll};

use fantoccini::Client;
use futures::future::BoxFuture;
use tower::Service;

use crate::context::{Request, Response};
use crate::Error;

#[derive(Clone)]
pub struct BrowserClient {
    id: u32,
    client: Client,
}

impl BrowserClient {
    pub fn new(id: u32, client: Client) -> Self {
        Self { id, client }
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub fn into_inner(self) -> Client {
        self.client
    }
}

impl fmt::Debug for BrowserClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.client, f)
    }
}

impl Deref for BrowserClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for BrowserClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

impl Service<Request> for BrowserClient {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, crate::Result<Response>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}
