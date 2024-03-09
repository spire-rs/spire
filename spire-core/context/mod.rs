//! [`Request`]'s [`Context`] and its extensions.
//!

use crate::backend::Backend;
pub use crate::context::body::Body;
use crate::context::body::Content;
use crate::context::extend::{Depth, Time};
pub use crate::context::extend::{Tag, Task, TaskBuilder};
pub use crate::context::queue::Queue;
pub use crate::context::signal::{IntoSignal, Signal};
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Datasets;
use crate::{BoxError, Error};

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
    response: Content<Response>,
    datasets: Datasets,
}

impl<B> Context<B> {
    /// Creates a new [`Context`].
    pub fn new(backend: B, datasets: Datasets, request: impl Into<Request>) -> Self {
        let mut request = request.into();
        request.extensions_mut().get_or_insert_with(Tag::default);
        request.extensions_mut().get_or_insert_with(Depth::default);
        request.extensions_mut().get_or_insert_with(Time::default);

        Self {
            backend,
            request,
            response: Content::None,
            datasets,
        }
    }

    pub async fn try_resolve(&mut self) -> Result<(), BoxError>
    where
        B: Backend,
    {
        // TODO.

        // if self.response.is_some() {
        //     return Ok(());
        // }

        let request = self.request.clone();
        let response = self.backend.try_resolve(request).await;
        let response = response.map_err(|x| x.into())?;
        // self.response.replace(response);

        Ok(())
    }

    pub fn queue(&self) -> Queue {
        let request = self.request.clone();
        let dataset = self.datasets.get::<Request>();
        Queue::new(request, dataset)
    }

    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.datasets.get::<T>()
    }

    /// Returns a reference to the [`Request`].
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Returns a reference to the [`Response`].
    pub fn response(&self) -> Option<&Response> {
        self.response.some()
    }
}
