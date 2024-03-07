use crate::backend::Backend;
pub use crate::context::body::Body;
pub use crate::context::extension::{Depth, Tag, Task, Time};
pub use crate::context::queue::Queue;
pub use crate::context::signal::{IntoSignal, Signal};

mod body;
mod extension;
mod queue;
mod signal;

/// Type alias for [`http::Request`] whose body type defaults to [`Body`].
pub type Request<B = Body> = http::Request<B>;
/// Type alias for [`http::Response`] whose body type defaults to [`Body`].
pub type Response<B = Body> = http::Response<B>;

/// Framework-specific [`Context`] type.
pub struct Context<B> {
    backend: B,
    request: Request,
    response: Option<Response>,
    queue: Option<Queue>,
}

impl<B> Context<B> {
    pub fn new(backend: B, request: impl Into<Request>) -> Self {
        let mut request = request.into();
        request.extensions_mut().get_or_insert_with(Tag::default);
        request.extensions_mut().get_or_insert_with(Depth::default);
        request.extensions_mut().get_or_insert_with(Time::default);

        Self {
            backend,
            request,
            response: None,
            queue: None,
        }
    }

    pub(crate) fn attach_queue(&mut self, queue: Queue) {
        self.queue = Some(queue);
    }

    pub async fn resolve(self)
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
