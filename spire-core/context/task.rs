use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};

use crate::context::Body;

pub type Request<B = Body> = http::Request<B>;
pub type Response<B = Body> = http::Response<B>;

/// To ensure the type-safe usage of [`Tag`]s and [`Task`]s inside of handlers,
/// you may want to create a custom enum, that implements `Into<Tag>` or `Into<Task>`:
///
/// ```rust
/// use spire_core::context::Task;
///
/// #[derive(Debug, Clone)]
/// pub enum Routes {
///     OnlyDiscoverLinks(String),
///     ExtractFromLink(String),
/// }
///
/// impl Into<Task> for Routes {
///     fn into(self) -> Task {
///         todo!()
///     }
/// }
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub enum Tag {
    /// Explicitly call the fallback handler.
    #[default]
    Fallback,
    Sequence(String),
    Rehash(u64),
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag::Sequence(value)
    }
}

impl From<u64> for Tag {
    fn from(value: u64) -> Self {
        Tag::Rehash(value)
    }
}

pub struct Task {
    request: Request,
    depth: Option<NonZeroUsize>,
}

impl Task {
    pub fn new(request: impl Into<Request>) -> Self {
        Self {
            request: request.into(),
            depth: None,
        }
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        let depth = NonZeroUsize::new(depth);
        self.depth = Some(depth.unwrap_or(NonZeroUsize::MIN));
        self
    }

    pub fn branch(&self, request: impl Into<Request>) -> Self {
        Self {
            request: request.into(),
            depth: self.depth.map(|x| x.saturating_add(1)),
        }
    }
}

impl Deref for Task {
    type Target = Request;

    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

impl DerefMut for Task {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.request
    }
}
