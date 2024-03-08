//! [`Request`]'s [`Context`] and its extensions.
//!

use crate::backend::Backend;
pub use crate::context::body::Body;
use crate::context::extend::{Depth, Time};
pub use crate::context::extend::{Tag, Task, TaskBuilder};
pub use crate::context::queue::Queue;
pub use crate::context::signal::{IntoSignal, Signal};
use crate::BoxError;

mod body;
mod extend;
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

/// Framework-specific context of the [`Request`].
pub struct Context<B> {
    backend: B,
    request: Request,
    response: Option<Response>,
    queue: Queue,
}

impl<B> Context<B> {
    /// Creates a new [`Context`].
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

    pub async fn try_resolve(&mut self) -> Result<(), BoxError>
    where
        B: Backend,
    {
        if self.response.is_some() {
            return Ok(());
        }

        let request = self.request.clone();
        let response = self.backend.try_resolve(request).await;
        let response = response.map_err(|x| x.into())?;
        self.response.replace(response);

        Ok(())
    }

    pub fn queue(&self) -> Queue {
        todo!()
    }

    /// Returns a reference to the attached tag.
    pub fn tag(&self) -> &Tag {
        let ext = self.request.extensions().get();
        ext.expect("tag should be present")
    }

    /// Returns a mutable reference to the attached tag.
    pub fn tag_mut(&mut self) -> &mut Tag {
        let ext = self.request.extensions_mut().get_mut();
        ext.expect("tag should be present")
    }

    /// Returns a recursive depth of this [`Request`].
    pub fn depth(&self) -> usize {
        self.request.depth()
    }
}
