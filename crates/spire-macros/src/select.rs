//! `#[derive(Select)]`.

use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Error, Field, Fields, GenericArgument, Ident, LitStr,
    PathArguments, Result, Token, Type,
};

/// The arguments of `#[select("css", attr = "name")]`, both optional.
struct SelectArgs {
    css: Option<LitStr>,
    attr: Option<LitStr>,
}

impl Parse for SelectArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let css = if input.peek(LitStr) {
            Some(input.parse()?)
        } else {
            None
        };

        let mut attr = None;
        while !input.is_empty() {
            if css.is_some() || attr.is_some() {
                input.parse::<Token![,]>()?;
                if input.is_empty() {
                    break;
                }
            }
            let key: Ident = input.parse()?;
            if key != "attr" {
                return Err(Error::new(key.span(), "expected `attr = \"...\"`"));
            }
            input.parse::<Token![=]>()?;
            attr = Some(input.parse()?);
        }

        Ok(Self { css, attr })
    }
}

/// Parses the `#[select(...)]` attribute among `attrs`, if any.
fn select_args(attrs: &[Attribute]) -> Result<Option<SelectArgs>> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("select")) {
        if found.is_some() {
            return Err(Error::new(attr.span(), "duplicate `#[select]` attribute"));
        }
        found = Some(attr.parse_args::<SelectArgs>()?);
    }
    Ok(found)
}

/// How many matches a field takes.
enum Arity<'a> {
    One(&'a Type),
    Optional(&'a Type),
    Many(&'a Type),
}

/// Classifies a field type as `T`, `Option<T>` or `Vec<T>`.
fn arity(ty: &Type) -> Arity<'_> {
    let Type::Path(path) = ty else {
        return Arity::One(ty);
    };
    let Some(segment) = path.path.segments.last() else {
        return Arity::One(ty);
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Arity::One(ty);
    };
    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    });

    match (types.next(), types.next()) {
        (Some(inner), None) if segment.ident == "Option" => Arity::Optional(inner),
        (Some(inner), None) if segment.ident == "Vec" => Arity::Many(inner),
        _ => Arity::One(ty),
    }
}

/// Quotes `Some(value)` or `None`.
fn option(value: Option<&LitStr>) -> proc_macro2::TokenStream {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
}

/// Generates the binding that extracts one field into a variable of its name.
fn expand_field(owner: &str, name: &Ident, field: &Field) -> Result<proc_macro2::TokenStream> {
    let Some(args) = select_args(&field.attrs)? else {
        return Err(Error::new(
            field.span(),
            "every field needs `#[select(\"css\")]` or `#[select(attr = \"...\")]`",
        ));
    };
    if args.css.is_none() && args.attr.is_none() {
        return Err(Error::new(
            field.span(),
            "`#[select]` needs a selector or `attr`",
        ));
    }

    let field_name = name.to_string();
    let values = format_ident!("__{}_values", name);
    let attr = option(args.attr.as_ref());
    let (selector, label) = match &args.css {
        Some(css) => (
            quote! {{
                static SELECTOR: ::spire::__private::OnceLock<::spire::__private::Selector> =
                    ::spire::__private::OnceLock::new();
                ::core::option::Option::Some(::spire::__private::cached(&SELECTOR, #css)?)
            }},
            css.value(),
        ),
        None => (quote!(::core::option::Option::None), ":scope".to_owned()),
    };

    let parse = |ty: &Type| quote!(::spire::__private::parse::<#ty>(#owner, #field_name, value));
    let value = match arity(&field.ty) {
        Arity::One(ty) => {
            let parse = parse(ty);
            quote! {
                match #values.first() {
                    ::core::option::Option::Some(value) => #parse?,
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(
                            ::spire::__private::missing(#owner, #field_name, #label),
                        );
                    }
                }
            }
        }
        Arity::Optional(ty) => {
            let parse = parse(ty);
            quote!(#values.first().map(|value| #parse).transpose()?)
        }
        Arity::Many(ty) => {
            let parse = parse(ty);
            quote! {
                #values
                    .iter()
                    .map(|value| #parse)
                    .collect::<::spire::Result<::std::vec::Vec<#ty>>>()?
            }
        }
    };

    Ok(quote! {
        let #values = ::spire::__private::values(element, #selector, #attr);
        let #name = #value;
    })
}

pub fn expand(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new(
            input.span(),
            "`Select` can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new(
            input.span(),
            "`Select` needs a struct with named fields",
        ));
    };

    let root = match select_args(&input.attrs)? {
        Some(SelectArgs {
            attr: Some(attr), ..
        }) => {
            return Err(Error::new(attr.span(), "`attr` is only allowed on fields"));
        }
        Some(SelectArgs { css, .. }) => option(css.as_ref()),
        None => option(None),
    };

    let owner = input.ident.to_string();
    let mut bindings = Vec::with_capacity(fields.named.len());
    let mut names = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        if let Some(name) = &field.ident {
            bindings.push(expand_field(&owner, name, field)?);
            names.push(name);
        }
    }

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::spire::extract::Select for #ident #ty_generics #where_clause {
            const ROOT: ::core::option::Option<&'static str> = #root;

            fn select(element: ::spire::__private::ElementRef<'_>) -> ::spire::Result<Self> {
                #(#bindings)*
                ::core::result::Result::Ok(Self { #(#names),* })
            }
        }
    })
}
