//! [`Request`]'s [`Context`] and its extensions.
//!

use std::future::Future;
use std::mem;

pub use body::Body;
use extend::{Depth, Time};
pub use extend::{Tag, Task, TaskBuilder};
pub use queue::Queue;
pub use signal::{IntoSignal, Query, Signal};

use crate::backend::Backend;
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Datasets;
use crate::{BoxError, Error, Result};

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

#[derive(Debug, Default)]
enum Exchange {
    #[default]
    None,
    Request(Request),
    Response(Response),
    Error(Error),
}

impl Exchange {
    pub async fn resolve<F, T>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(Request) -> T,
        T: Future<Output = Result<Response, BoxError>>,
    {
        if !matches!(self, Exchange::Request(_)) {
            return Ok(());
        };

        let x = match mem::take(self) {
            Exchange::Request(x) => f(x).await,
            _ => unreachable!(),
        };

        *self = match x {
            Ok(x) => Exchange::Response(x),
            Err(x) => Exchange::Error(Error::new(x)),
        };

        Ok(())
    }

    pub fn take_request(&mut self) -> Option<Request> {
        if !matches!(self, Exchange::Request(_)) {
            return None;
        }

        match mem::take(self) {
            Exchange::Request(x) => Some(x),
            _ => None,
        }
    }

    pub fn take_response(&mut self) -> Option<Response> {
        if !matches!(self, Exchange::Response(_)) {
            return None;
        }

        match mem::take(self) {
            Exchange::Response(x) => Some(x),
            _ => None,
        }
    }

    pub fn take_error(&mut self) -> Option<Error> {
        if !matches!(self, Exchange::Error(_)) {
            return None;
        }

        match mem::take(self) {
            Exchange::Error(x) => Some(x),
            _ => None,
        }
    }
}

// TODO.
/// Framework-specific context of the [`Request`].
pub struct Context<B> {
    // todo: see backend::exchange
    queue: Queue,
    datasets: Datasets,
    exchange: Exchange,
    backend: B,
}

impl<B> Context<B> {
    /// Creates a new [`Context`].
    pub fn new(backend: B, datasets: Datasets, mut request: Request) -> Self {
        request.extensions_mut().get_or_insert_with(Tag::default);
        request.extensions_mut().get_or_insert_with(Depth::default);
        request.extensions_mut().get_or_insert_with(Time::default);
        let dataset = datasets.get::<Request>();

        // TODO: lazy Queue.
        Self {
            queue: Queue::new(dataset, request.depth()),
            exchange: Exchange::Request(request),
            backend,
            datasets,
        }
    }

    pub async fn try_resolve(&mut self) -> Result<()>
    where
        B: Backend,
    {
        // TODO: Replace with some kind of backend wrapper.
        let fut = self.exchange.resolve(|x| async {
            let fut = self.backend.call(x).await;
            fut.map_err(|x| -> BoxError { x.into() })
        });

        fut.await
    }

    pub async fn resolve(&mut self)
    where
        B: Backend,
    {
        let _ = self.try_resolve().await;
    }

    pub fn request_ref(&self) -> Option<&Request> {
        match &self.exchange {
            Exchange::Request(x) => Some(x),
            _ => None,
        }
    }

    pub fn request_mut(&mut self) -> Option<&Request> {
        todo!()
    }

    pub fn response_ref(&self) -> Option<&Request> {
        todo!()
    }

    pub fn response_mut(&mut self) -> Option<&Request> {
        todo!()
    }

    pub fn queue(&self) -> Queue {
        self.queue.clone()
    }

    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.datasets.get::<T>()
    }
}
