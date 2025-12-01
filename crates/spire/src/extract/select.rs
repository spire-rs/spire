use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Represents a tag identifier for selecting HTML attributes.
///
/// This is used as a key in attribute selection when parsing HTML elements.
#[derive(Debug, Clone)]
pub struct AttrTag(String);

/// Represents the data extracted from an HTML attribute.
///
/// This is the value associated with an [`AttrTag`] after parsing.
#[derive(Debug, Clone)]
pub struct AttrData(String);

// TODO: Vec<AttrTag> to &'static [AttrTag].

/// Trait for types that can be constructed from selected HTML attributes.
///
/// This trait enables declarative extraction of structured data from HTML markup
/// by defining required and optional attributes and how to parse them.
///
/// Can be automatically generated with a `Select` derive macro:
///
/// ```rust,ignore
/// // This requires the "macros" feature to be enabled
/// use spire::extract::Select;
///
/// #[derive(Debug, Select)]
/// struct SelectAttr {}
/// ```
pub trait Select {
    /// Returns a list of selectable attributes required to build [`Self`].
    fn list_required_attributes() -> Vec<AttrTag>;

    /// Returns a list of selectable attributes optional to build [`Self`].
    fn list_optional_attributes() -> Vec<AttrTag>;

    /// Builds a new [`Self`] from the selectable attributes.
    fn parse_from_attributes(attr: HashMap<AttrTag, AttrData>) -> Self;
}

/// Declarative markup extractor for structured HTML data.
///
/// `Elements<T>` extracts and parses HTML elements into a structured type `T`
/// that implements the [`Select`] trait. This allows for type-safe extraction
/// of data from HTML markup.
///
/// # Examples
///
/// ```ignore
/// use spire::extract::{Elements, Select};
///
/// #[derive(Debug, Select)]
/// struct Product {
///     name: String,
///     price: f64,
/// }
///
/// async fn handler(Elements(product): Elements<Product>) {
///     println!("Product: {} costs ${}", product.name, product.price);
/// }
/// ```
#[must_use]
#[derive(Clone)]
pub struct Elements<T>(pub T);

impl<T> Elements<T> {
    /// Creates a new [`Elements`] instance from an iterator of element tags.
    ///
    /// This method is used internally to construct the extractor from parsed HTML elements.
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
