use std::ops::{Deref, DerefMut};

/// TODO.
pub trait Select {
    /// TODO.
    fn list_selected() -> Vec<String>;

    /// TODO.
    fn from_list(selected: &[String]) -> Self;
}

/// TODO.
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
