#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
// #![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput, GenericParam};

// use proc_macro2::TokenStream as TokenStream2;

/// Automatically generates the `Select` trait for a `struct`.
#[proc_macro_derive(Select)]
pub fn derive_select(input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);

    // Add a bound `T: Select` to every type parameter `T`.
    for param in &mut input.generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(spire::extract::Select));
        }
    }

    let (impl_ts, ty_ts, where_clause) = input.generics.split_for_impl();
    let name = input.ident;

    // TODO: Implement extended macro.
    let expanded = quote! {
        impl #impl_ts spire::extract::Select for #name #ty_ts #where_clause {
            fn list_required_attributes() -> Vec<AttrTag> {
                todo!()
            }

            fn list_optional_attributes() -> Vec<AttrTag> {
                todo!()
            }

            fn parse_from_attributes(attr: HashMap<AttrTag, AttrData>) -> Self {
                todo!()
            }
        }
    };

    TokenStream::from(expanded)
}
