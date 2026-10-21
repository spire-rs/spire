use std::future::{Future, ready};

use bytes::Bytes;

use crate::Result;
use crate::backend::{Backend, Client};
use crate::request::{Request, Response};

/// A backend whose client answers every request with an empty `200 OK`.
///
/// Useful for tests and examples that exercise the crawler without network
/// access.
#[derive(Debug, Clone, Copy, Default)]
pub struct Noop;

impl Backend for Noop {
    type Client = Self;

    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send {
        ready(Ok(Self))
    }
}

impl Client for Noop {
    fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
        ready(Ok(Response::default()))
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        ready(Ok(Bytes::new()))
    }
}
