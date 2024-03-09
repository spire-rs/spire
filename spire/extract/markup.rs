use std::convert::Infallible;

use spire_core::backend::Backend;
use spire_core::context::Context;
#[cfg(feature = "macros")]
pub use spire_macros::extract::{Select, Selector};

use crate::extract::FromContextParts;

pub mod transform {
    // TODO: also use for Text?
    pub trait Transform<T> {
        type Output;

        fn transform(input: T) -> Self::Output;
    }

    pub struct Normal;

    impl Transform<()> for Normal {
        type Output = ();

        fn transform(input: ()) -> Self::Output {
            input
        }
    }

    pub struct Reduce;

    impl Transform<()> for Reduce {
        type Output = ();

        fn transform(input: ()) -> Self::Output {
            input
        }
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Html<T = transform::Normal>(pub T::Output)
where
    T: transform::Transform<()>;

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Html<T>
where
    B: Backend,
    T: transform::Transform<()>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Selector<T>
where
    B: Backend,
    S: Sync,
    T: Select,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let html = Html::<transform::Normal>::from_context_parts(cx, state).await;
        todo!()
    }
}
