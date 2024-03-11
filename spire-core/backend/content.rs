use crate::BoxError;

// TODO: do i even need it
/// [`Option`]
#[derive(Debug, Default)]
pub enum Content<T> {
    #[default]
    None,
    Response(T),
    Error(BoxError),
}

impl<T> Content<T> {
    pub fn map<F, T2>(self, f: F) -> Content<T2>
    where
        F: FnOnce(T) -> T2,
    {
        match self {
            Content::Response(x) => Content::Response(f(x)),
            Content::None => Content::None,
            Content::Error(x) => Content::Error(x),
        }
    }

    pub fn some(&self) -> Option<&T> {
        match self {
            Content::Response(x) => Some(x),
            _ => None,
        }
    }
}
