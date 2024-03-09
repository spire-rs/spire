use std::ops::{Deref, DerefMut};

pub trait Select {
    fn list_selected() -> Vec<String>;

    fn from_list(selected: &[String]) -> Self;
}

pub struct Selector<T>(pub T);

impl<T> Selector<T> {
    pub fn new() -> Self
    where
        T: Select,
    {
        todo!()
    }
}

impl<T> Deref for Selector<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Selector<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
