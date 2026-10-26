//! `#[derive(FromRef)]`.

use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, Result};

/// Whether `field` has `#[from_ref(skip)]`, rejecting any other argument.
fn skipped(field: &syn::Field) -> Result<bool> {
    let mut skip = false;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("from_ref"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
                Ok(())
            } else {
                Err(meta.error("expected `skip`"))
            }
        })?;
    }
    Ok(skip)
}

pub fn expand(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new(
            input.span(),
            "`FromRef` can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new(
            input.span(),
            "`FromRef` can only be derived for structs with named fields",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "`FromRef` cannot be derived for generic structs",
        ));
    }

    let state = &input.ident;
    let mut impls = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        if skipped(field)? {
            continue;
        }
        let name = &field.ident;
        let ty = &field.ty;
        impls.push(quote! {
            impl ::spire::extract::FromRef<#state> for #ty {
                fn from_ref(state: &#state) -> Self {
                    ::core::clone::Clone::clone(&state.#name)
                }
            }
        });
    }
    Ok(quote!(#(#impls)*))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(input: &str) -> Result<String> {
        expand(&syn::parse_str(input)?).map(|tokens| tokens.to_string())
    }

    #[test]
    fn implements_from_ref_for_every_field_but_skipped_ones() {
        let expanded =
            expand_str("struct App { db: Pool, #[from_ref(skip)] key: String, agent: Agent }")
                .unwrap();
        assert!(expanded.contains("FromRef < App > for Pool"), "{expanded}");
        assert!(expanded.contains("FromRef < App > for Agent"), "{expanded}");
        assert!(!expanded.contains("for String"), "{expanded}");
    }

    #[test]
    fn rejects_what_it_cannot_derive() {
        for input in [
            "enum App { A }",
            "struct App(Pool);",
            "struct App<T> { db: T }",
            "struct App { #[from_ref(clone)] db: Pool }",
        ] {
            assert!(expand_str(input).is_err(), "{input} should be rejected");
        }
    }
}
