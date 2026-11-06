//! Checks shared by the derives, on the item they are derived for and its
//! attributes, so they read and fail alike.

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Error, FieldsNamed, Result, Variant};

/// The `struct`, `enum` or `union` keyword of `input`. Unlike `input.span()`,
/// neither doc comments nor a visibility move it.
pub fn item_keyword(input: &DeriveInput) -> Span {
    match &input.data {
        Data::Struct(data) => data.struct_token.span,
        Data::Enum(data) => data.enum_token.span,
        Data::Union(data) => data.union_token.span,
    }
}

/// Parses the `#[name(...)]` attribute among `attrs` with `parse`, if there is
/// one, rejecting a second.
pub fn single<'a, T>(
    attrs: &'a [Attribute],
    name: &str,
    parse: impl Fn(&Attribute) -> Result<T>,
) -> Result<Option<(&'a Attribute, T)>> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident(name)) {
        if found.is_some() {
            let message = format!("duplicate `#[{name}]` attribute");
            return Err(Error::new_spanned(attr, message));
        }
        found = Some((attr, parse(attr)?));
    }
    Ok(found)
}

/// Rejects `#[name]` among `attrs`, an item's own, as it belongs on its
/// `members`, e.g. `"fields"`.
pub fn forbid(attrs: &[Attribute], name: &str, members: &str) -> Result<()> {
    match attrs.iter().find(|attr| attr.path().is_ident(name)) {
        Some(attr) => {
            let message = format!("`#[{name}]` is only allowed on {members}");
            Err(Error::new_spanned(attr, message))
        }
        None => Ok(()),
    }
}

/// Rejects generic parameters on `input`, a `kind` such as `"structs"`.
pub fn no_generics(input: &DeriveInput, derive: &str, kind: &str) -> Result<()> {
    if input.generics.params.is_empty() {
        Ok(())
    } else {
        let message = format!("`{derive}` cannot be derived for generic {kind}");
        Err(Error::new_spanned(&input.generics, message))
    }
}

/// The named fields of `input`, which must be a struct that has them.
pub fn named_struct<'a>(input: &'a DeriveInput, derive: &str) -> Result<&'a FieldsNamed> {
    let Data::Struct(data) = &input.data else {
        let message = format!("`{derive}` can only be derived for structs");
        return Err(Error::new(item_keyword(input), message));
    };
    let syn::Fields::Named(fields) = &data.fields else {
        let message = format!("`{derive}` can only be derived for structs with named fields");
        return Err(Error::new(item_keyword(input), message));
    };
    Ok(fields)
}

/// The variants of `input`, which must be a non-generic enum of unit variants.
pub fn unit_enum<'a>(
    input: &'a DeriveInput,
    derive: &str,
) -> Result<&'a Punctuated<Variant, Comma>> {
    let Data::Enum(data) = &input.data else {
        let message = format!("`{derive}` can only be derived for enums");
        return Err(Error::new(item_keyword(input), message));
    };
    no_generics(input, derive, "enums")?;
    if let Some(variant) = data
        .variants
        .iter()
        .find(|variant| !matches!(variant.fields, syn::Fields::Unit))
    {
        let message = format!("`{derive}` variants cannot have fields");
        return Err(Error::new_spanned(&variant.fields, message));
    }
    Ok(&data.variants)
}

/// Quotes `Some(value)`, or `None`.
pub fn option(value: Option<impl ToTokens>) -> TokenStream {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
}
