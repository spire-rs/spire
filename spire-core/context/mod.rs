use crate::backend::Backend;
use crate::BoxError;
pub use crate::context::body::Body;
pub use crate::context::extension::{Depth, Tag, Task, Time};
pub use crate::context::queue::Queue;
pub use crate::context::signal::{IntoSignal, Signal};
use crate::dataset::Dataset;

mod body;
mod extension;
mod queue;
mod signal;

/// Type alias for `http::`[`Request`] whose body type defaults to [`Body`].
///
/// [`Request`]: http::Request
pub type Request<B = Body> = http::Request<B>;
/// Type alias for `http::`[`Response`] whose body type defaults to [`Body`].
///
/// [`Response`]: http::Response
pub type Response<B = Body> = http::Response<B>;



pub struct Error {
    inner: BoxError,
}

impl Error {
    /// Creates a new [`Error`] from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        let inner = error.into();
        Self { inner }
    }

    /// Returns the underlying boxed error.
    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

/// Framework-specific [`Context`] type.
pub struct Context<B> {
    backend: B,
    request: Request,
    response: Option<Response>,
    queue: Queue,
}

impl<B> Context<B> {
    pub fn new(backend: B, queue: Queue, request: impl Into<Request>) -> Self {
        let mut request = request.into();
        request.extensions_mut().get_or_insert_with(Tag::default);
        request.extensions_mut().get_or_insert_with(Depth::default);
        request.extensions_mut().get_or_insert_with(Time::default);

        Self {
            backend,
            request,
            response: None,
            queue,
        }
    }

    pub async fn resolve(self) -> Result<(), BoxError>
    where
        B: Backend,
    {
        let response = self.backend.resolve(self.request).await;
        todo!()
    }

    pub fn tag(&self) -> &Tag {
        let ext = self.request.extensions().get::<Tag>();
        ext.expect("extension should be present")
    }

    pub fn depth(&self) -> &Depth {
        let ext = self.request.extensions().get::<Depth>();
        ext.expect("extension should be present")
    }

    pub fn time(&self) -> &Time {
        let ext = self.request.extensions().get::<Time>();
        ext.expect("extension should be present")
    }
}
