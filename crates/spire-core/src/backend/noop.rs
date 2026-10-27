use std::future::{Future, ready};

use crate::Result;
use crate::backend::{Backend, Session};
use crate::http::{Request, Response};

/// A backend whose client answers every request with an empty `200 OK`.
///
/// Useful for tests and examples that exercise the crawler without network
/// access.
#[derive(Debug, Clone, Copy, Default)]
pub struct Noop;

impl Backend for Noop {
    type Session = Self;

    fn connect(&self) -> impl Future<Output = Result<Self::Session>> + Send {
        ready(Ok(Self))
    }
}

impl Session for Noop {
    fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response>> + Send {
        ready(Ok(Response::default()))
    }
}
