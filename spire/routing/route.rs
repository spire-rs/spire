use std::convert::Infallible;
use std::sync::Mutex;

use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use spire_core::collect::HandlerContext;

use crate::handler::{ControlFlow, Handler, IntoFlow};

pub struct Route<E = Infallible> {
    srv: Mutex<BoxCloneService<HandlerContext, ControlFlow, E>>,
}

impl<E> Route<E> {
    pub(crate) fn new<T>(svc: T) -> Self
    where
        T: Service<HandlerContext, Error = E> + Clone + Send + 'static,
        T::Response: IntoFlow + 'static,
        T::Future: Send + 'static,
    {
        let svc = BoxCloneService::new(svc.map_response(IntoFlow::into_flow));

        Self {
            srv: Mutex::new(svc),
        }
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
