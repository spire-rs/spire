use std::convert::Infallible;
use std::marker::PhantomData;

use crate::extract::FromContextParts;
use crate::handler::HandlerContext;

pub struct Hyper;

pub struct Reqwest;

pub struct ClientHandler<T> {
    marker: PhantomData<T>,
}

pub struct Client<T>(pub ClientHandler<T>);

impl<T> Client<T> {}

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Client<T> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
