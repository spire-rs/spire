use std::convert::Infallible;
use std::sync::Mutex;

use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use spire_core::{IntoSignal, Signal};

use crate::handler::HandlerContext;

pub struct Route<E = Infallible> {
    srv: Mutex<BoxCloneService<HandlerContext, Signal, E>>,
}

impl<E> Route<E> {
    pub(crate) fn new<T>(svc: T) -> Self
    where
        T: Service<HandlerContext, Error = E> + Clone + Send + 'static,
        T::Response: IntoSignal + 'static,
        T::Future: Send + 'static,
    {
        let srv = Mutex::new(BoxCloneService::new(
            svc.map_response(IntoSignal::into_signal),
        ));

        Self { srv }
    }
}

// pub struct BoxRoute<S, E = Infallible> {
//     state: PhantomData<S>,
//     route: Route<E>,
// }
//
// impl<S, E> BoxRoute<S, E>
// where
//     S: Clone + Send + Sync + 'static,
// {
//     pub(crate) fn new<H, T>(handler: H)
//     where
//         H: Handler<T, S>,
//     {
//     }
// }
//
// impl<S, E> BoxRoute<S, E>
//     where
//         S: Clone + Send + Sync + 'static,
// {
// }
