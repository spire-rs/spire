#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod revisit;
mod select;
mod tags;

use proc_macro::TokenStream;
use syn::{DeriveInput, Error, parse_macro_input};

/// Derives `spire::extract::html::Select` from CSS selectors.
///
/// See the documentation of `spire::extract::html::Select` for the attributes.
#[proc_macro_derive(Select, attributes(select))]
pub fn derive_select(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    select::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives `spire::request::Revisit` for an enum of unit variants: a
/// variant with `#[revisit("1d")]` is crawled again that long after, one
/// without is crawled once.
///
/// Intervals combine days (`d`), hours (`h`), minutes (`m`) and seconds
/// (`s`), such as `"90s"`, `"30m"`, `"1h30m"` or `"1d"`, and must be above
/// zero; they are checked at compile time.
#[proc_macro_derive(Revisit, attributes(revisit))]
pub fn derive_revisit(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    revisit::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives `spire::request::Tags` for an enum of unit variants.
///
/// See the documentation of `spire::request::Tags` for the attributes.
#[proc_macro_derive(Tags, attributes(tag))]
pub fn derive_tags(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    tags::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
