use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use tower_service::Service;

pub use error::{Error, Result, Signal};
use inner::CollectorInner;
pub use types::{Context, Metrics, Request, Response};

mod error;
mod inner;
mod types;

pub struct Collector<C, R, B>(Arc<CollectorInner<C, R, B>>);

impl<C, R, B> Collector<C, R, B> {
    pub fn new<CT, RT>(worker: C, router: R) -> Collector<C, R, B>
    where
        C: Service<(), Response = CT, Error = Infallible>,
        R: Service<(), Response = RT, Error = Infallible>,
        CT: Service<Request<B>, Response = Response<()>, Error = Signal>,
        RT: Service<Context<B, B>, Response = Signal, Error = Infallible>,
    {
        todo!()
    }

    pub fn with<C2, CT>(self, crawler: C2) -> Collector<C2, R, B>
    where
        C2: Service<(), Response = C2, Error = Infallible>,
        CT: Service<Request<B>, Response = Response<()>, Error = Signal>,
    {
        todo!()
    }

    pub fn route<R2, RT>(self, router: R2) -> Collector<C, R2, B>
    where
        R2: Service<(), Response = RT, Error = Infallible>,
        RT: Service<Context<B>, Response = Signal, Error = Infallible>,
    {
        todo!()
    }

    pub async fn add<T, U>(&self, tasks: T)
    where
        T: IntoIterator<Item = U>,
        U: Into<Request<B>>,
    {
        for task in tasks.into_iter() {
            self.0.add(task.into()).await;
        }
    }
}

impl<C, R, CT, RT, B> Collector<R, C, B>
where
    C: Service<(), Response = CT, Error = Infallible>,
    R: Service<(), Response = RT, Error = Infallible>,
    CT: Service<Request<B>, Response = Response<B>, Error = Signal>,
    RT: Service<Context<B>, Response = Signal, Error = Infallible>,
{
    pub async fn run(&self) -> Result<Metrics> {
        todo!()
    }

    pub fn abort(&self) -> Result<()> {
        todo!()
    }
}

impl<C, R, B> fmt::Debug for Collector<C, R, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Collector").finish_non_exhaustive()
    }
}

impl<C, R, B> Clone for Collector<C, R, B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
