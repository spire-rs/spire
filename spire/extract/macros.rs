use std::convert::Infallible;

use spire_core::backend::Backend;
use spire_core::context::Context;
pub use spire_macros::extract::{Select, Selector};

use crate::extract::{FromContextParts, Html};

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Selector<T>
where
    B: Backend,
    S: Sync,
    T: Select,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let html = Html::from_context_parts(cx, state).await;
        todo!()
    }
}
