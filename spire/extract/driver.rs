use core::fmt;
use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use deadpool::managed::Object;
use fantoccini::Client;

use spire_core::driver::DriverManager;

use crate::extract::FromContextParts;
use crate::handler::HandlerContext;

mod sealed {
    use spire_core::driver::Browser;

    use super::{Chrome, Edge, Firefox, Safari};

    // TODO: Version, name, stats?
    pub trait Marker {
        fn browser() -> Browser;
    }

    impl Marker for Chrome {
        fn browser() -> Browser {
            Browser::Chrome
        }
    }

    impl Marker for Edge {
        fn browser() -> Browser {
            todo!()
        }
    }

    impl Marker for Firefox {
        fn browser() -> Browser {
            Browser::Firebox
        }
    }

    impl Marker for Safari {
        fn browser() -> Browser {
            Browser::Safari
        }
    }
}

pub struct Chrome;
pub struct Edge;
pub struct Firefox;
pub struct Safari;

/// Implements [`Deref`] and [`DerefMut`] traits with a [`Client`] target.
pub struct BrowserHandler<T> {
    marker: PhantomData<T>,
    client: Object<DriverManager>,
}

impl<T> Deref for BrowserHandler<T> {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        self.client.deref()
    }
}

impl<T> DerefMut for BrowserHandler<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client.deref_mut()
    }
}

impl<T> fmt::Debug for BrowserHandler<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserHandler").finish_non_exhaustive()
    }
}

pub struct Browser<T = Chrome>(pub BrowserHandler<T>);

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Browser<T> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

impl<T> fmt::Debug for Browser<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Browser").finish_non_exhaustive()
    }
}
