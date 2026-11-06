//! `#[derive(FromRef)]`.

use quote::quote;
use syn::{Attribute, Error, Field, Result};

use crate::input;

/// Parses `#[from_ref(skip)]`, the only argument, given once.
fn parse_skip(attr: &Attribute) -> Result<()> {
    let mut skip = false;
    attr.parse_nested_meta(|meta| {
        if !meta.path.is_ident("skip") {
            return Err(meta.error("expected `skip`"));
        }
        if skip {
            return Err(meta.error("duplicate `skip`"));
        }
        skip = true;
        Ok(())
    })?;
    if skip {
        Ok(())
    } else {
        Err(Error::new_spanned(attr, "expected `skip`"))
    }
}

/// Whether `field` has `#[from_ref(skip)]`.
fn skipped(field: &Field) -> Result<bool> {
    Ok(input::single(&field.attrs, "from_ref", parse_skip)?.is_some())
}

pub fn expand(input: &syn::DeriveInput) -> Result<proc_macro2::TokenStream> {
    input::forbid(&input.attrs, "from_ref", "fields")?;
    let fields = input::named_struct(input, "FromRef")?;
    input::no_generics(input, "FromRef", "structs")?;

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
            "struct App { #[from_ref()] db: Pool }",
            "#[from_ref(skip)] struct App { db: Pool }",
        ] {
            assert!(expand_str(input).is_err(), "{input} should be rejected");
        }
    }
}
