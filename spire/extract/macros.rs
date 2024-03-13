use spire_core::backend::Backend;
use spire_core::context::Context;
use spire_core::Error;
pub use spire_macros::extract::{Elements, Select};

use crate::extract::{FromContext, Html};

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Elements<T>
where
    B: Backend,
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let _ = Html::from_context(cx, state).await?;

        todo!()
    }
}
