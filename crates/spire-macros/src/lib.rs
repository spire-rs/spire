#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

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
