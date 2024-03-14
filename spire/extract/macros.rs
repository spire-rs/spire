use tower::Service;

use spire_core::context::{Context, Request, Response};
use spire_core::Error;
pub use spire_macros::extract::{Elements, Select};

use crate::extract::{FromContext, Html};

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Elements<T>
where
    B: Service<Request, Response = Response, Error = Error> + Send + Sync + 'static,
    <B as Service<Request>>::Future: Send,
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Html(html) = Html::from_context(cx, state).await?;
        todo!()
    }
}
