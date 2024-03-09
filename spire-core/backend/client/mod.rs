use std::fmt;
use std::sync::Mutex;

use tower::Service;
use tower::util::BoxCloneService;

use crate::backend::Backend;
use crate::BoxError;
use crate::context::{Request, Response};

pub struct HttpClient {
    inner: HttpClientInner
}

struct HttpClientInner {
    svc: Mutex<BoxCloneService<Request, Response, BoxError>>,
}

impl HttpClient {
    pub fn new<S, E>(svc: S) -> Self
    where
        S: Service<Request, Response = Response, Error = E> + Clone,
        E: Into<BoxError>,
    {
        todo!()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        // Self::new()
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
