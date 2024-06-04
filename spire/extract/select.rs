use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// TODO.
#[derive(Debug, Clone)]
pub struct AttrTag(String);

/// TODO.
#[derive(Debug, Clone)]
pub struct AttrData(String);

/// TODO.
///
/// Can be automatically generated with a `Select` derive macro:
///
/// ```rust
/// use spire::extract::Select;
///
/// #[derive(Debug, Select)]
/// struct SelectAttr {}
/// ```
pub trait Select {
    /// Returns a list of selectable attributes required to build [`Self`].
    fn list_required_attributes() -> &'static [AttrTag];

    /// Returns a list of selectable attributes optional to build [`Self`].
    fn list_optional_attributes() -> &'static [AttrTag];

    /// Builds a new [`Self`] from the selectable attributes.
    fn parse_from_attributes(attr: HashMap<AttrTag, AttrData>) -> Self;
}

/// Declarative markup extractor.
#[must_use]
#[derive(Clone)]
pub struct Elements<T>(pub T);

impl<T> Elements<T> {
    /// Creates a new [`Elements`].
    pub fn new<U>(tags: U) -> Self
    where
        U: IntoIterator<Item = ()>,
        T: Select,
    {
        let _ = tags.into_iter();

        todo!()
    }
}

impl<T> Deref for Elements<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Elements<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> fmt::Debug for Elements<T>
where
    T: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
