// todo: rename

use std::fmt;

use crate::backend::Backend;
use crate::context::{Request, Response};
use crate::Error;

#[derive(Clone)]
pub struct Exchange<B> {
    backend: B,
}

#[derive(Default)]
enum Status {
    #[default]
    None,
    Request(Request),
    Response(Response),
    Error(Error),
}

impl<B> Exchange<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B> fmt::Debug for Exchange<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
