use std::fmt;
use std::fmt::Formatter;
use std::ops::{Deref, DerefMut};

/// TODO.
pub trait Select {
    /// Returns a list of selectable attributes.
    fn list_selected() -> Vec<String>;

    /// TODO.
    fn from_list(selected: &[String]) -> Self;
}

/// Declarative markup extractor.
#[derive(Clone)]
pub struct Elements<T>(pub T);

impl<T> Elements<T> {
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

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Elements<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> fmt::Debug for Elements<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
