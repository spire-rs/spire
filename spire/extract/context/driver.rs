use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use spire_core::backend::{BrowserClient, BrowserPool};
use spire_core::context::Context;
use spire_core::Error;
#[cfg(feature = "macros")]
use spire_macros::extract::{Elements, Select};

use crate::extract::FromContextRef;

// TODO: Snapshot, Screen, Color, Capture.

#[async_trait::async_trait]
impl<S> FromContextRef<BrowserPool, S> for BrowserClient {
    type Rejection = Error;

    async fn from_context_parts(
        cx: &Context<BrowserPool>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        cx.client().await
    }
}

pub struct BrowserHandle<T> {
    client: BrowserClient,
    marker: PhantomData<T>,
}

impl<T> Deref for BrowserHandle<T> {
    type Target = BrowserClient;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<T> DerefMut for BrowserHandle<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

pub struct Browser<T>(pub BrowserHandle<T>);

#[async_trait::async_trait]
impl<S, T> FromContextRef<BrowserPool, S> for Browser<T> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<BrowserPool>, _: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct View(pub ());

#[async_trait::async_trait]
impl<S> FromContextRef<BrowserPool, S> for View {
    type Rejection = Error;

    async fn from_context_parts(cx: &Context<BrowserPool>, _: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

impl Deref for View {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for View {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "macros")]
#[async_trait::async_trait]
impl<S, T> FromContextRef<BrowserPool, S> for Elements<T>
where
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context_parts(cx: &Context<BrowserPool>, _: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
