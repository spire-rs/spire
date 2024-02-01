use std::sync::Arc;

use crate::handler::Handler;
use crate::routing::fallback::Fallback;
pub use crate::routing::path_router::Label;
use crate::routing::path_router::PathRouter;

mod fallback;
mod path_router;
mod route;

#[must_use]
pub struct Router<S = ()> {
    inner: Arc<RouterInner<S>>,
}

struct RouterInner<S> {
    routes: PathRouter<S>,
    fallback: Fallback<S>,
}

impl<S> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        let inner = RouterInner {
            routes: PathRouter::default(),
            fallback: Fallback::default(),
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn route<L, H, T>(self, label: L, handler: H) -> Self
    where
        L: Into<Label>,
        H: Handler<T, S>,
        T: 'static,
    {
        let label = label.into();
        todo!()
    }

    pub fn nest(self, other: Router<S>) -> Self {
        todo!()
    }

    pub fn merge(self, other: Router<S>) -> Self {
        todo!()
    }

    pub fn fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        todo!()
    }

    pub fn with_state<S2>(self, state: S) -> Router<S2> {
        todo!()
    }

    fn map_inner<F, S2>(self, f: F) -> Router<S2>
    where
        F: FnOnce(RouterInner<S>) -> RouterInner<S2>,
    {
        Router {
            inner: Arc::new(f(self.into_inner())),
        }
    }

    fn map_inner_mut<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut RouterInner<S>),
    {
        let mut inner = self.into_inner();
        f(&mut inner);
        Router {
            inner: Arc::new(inner),
        }
    }

    fn into_inner(self) -> RouterInner<S> {
        Arc::try_unwrap(self.inner).unwrap_or_else(|arc| RouterInner {
            routes: arc.routes.clone(),
            fallback: arc.fallback.clone(),
        })
    }
}

impl<S> Default for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use crate::extract::{FromRef, State};
    use crate::handler::Handler;
    use crate::routing::{Label, Router};

    #[test]
    fn basic_routing() {
        async fn handler() {}
        let router: Router<()> = Router::default()
            .route(Label::default(), handler)
            .route(Label::default(), || async {})
            .fallback(handler)
            .with_state(());
    }

    #[test]
    fn state_routing() {
        #[derive(Debug, Default, Clone)]
        struct AppState {
            sub: u32,
        }

        impl FromRef<AppState> for u32 {
            fn from_ref(input: &AppState) -> Self {
                input.sub.clone()
            }
        }

        async fn handler(State(_): State<AppState>, State(_): State<u32>) {}

        let router: Router<AppState> = Router::default()
            .route(Label::default(), handler)
            .with_state(AppState::default());
    }
}
