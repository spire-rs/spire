//! `#[derive(Tags)]`.

use std::collections::HashMap;

use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Error, Fields, Ident, LitStr, Result, Token};

/// The argument of `#[tag(rename_all = "...")]` or `#[tag(rename = "...")]`.
struct TagArg {
    key: Ident,
    value: LitStr,
}

impl Parse for TagArg {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key = input.parse()?;
        input.parse::<Token![=]>()?;
        let value = input.parse()?;
        input.parse::<Option<Token![,]>>()?;
        Ok(Self { key, value })
    }
}

/// Parses the `#[tag(key = "...")]` attribute among `attrs`, if any.
fn tag_arg(attrs: &[Attribute], expected: &str) -> Result<Option<LitStr>> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("tag")) {
        if found.is_some() {
            return Err(Error::new(attr.span(), "duplicate `#[tag]` attribute"));
        }
        let arg = attr.parse_args::<TagArg>()?;
        if arg.key != expected {
            let message = format!("expected `{expected} = \"...\"`");
            return Err(Error::new(arg.key.span(), message));
        }
        found = Some(arg.value);
    }
    Ok(found)
}

/// How variant names become tags, as in serde's `rename_all`.
#[derive(Clone, Copy)]
enum Case {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl Case {
    fn parse(value: &LitStr) -> Result<Self> {
        Ok(match value.value().as_str() {
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "PascalCase" => Self::Pascal,
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnake,
            "kebab-case" => Self::Kebab,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebab,
            _ => {
                return Err(Error::new(
                    value.span(),
                    "expected one of `lowercase`, `UPPERCASE`, `PascalCase`, `camelCase`, \
                     `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case`, `SCREAMING-KEBAB-CASE`",
                ));
            }
        })
    }

    /// Renames a `PascalCase` variant name.
    fn apply(self, name: &str) -> String {
        let words = words(name);
        let join = |separator: &str, upper: bool| {
            let words = words.iter().map(|word| {
                if upper {
                    word.to_uppercase()
                } else {
                    word.to_lowercase()
                }
            });
            words.collect::<Vec<_>>().join(separator)
        };

        match self {
            Self::Lower => join("", false),
            Self::Upper => join("", true),
            Self::Pascal => words.iter().map(|word| capitalize(word)).collect(),
            Self::Camel => {
                let pascal: String = words.iter().map(|word| capitalize(word)).collect();
                let mut chars = pascal.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(chars).collect()
                })
            }
            Self::Snake => join("_", false),
            Self::ScreamingSnake => join("_", true),
            Self::Kebab => join("-", false),
            Self::ScreamingKebab => join("-", true),
        }
    }
}

/// Splits `HttpPage2Item`, `HTTPPage` or `Snake_Name` into words.
fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let previous = i.checked_sub(1).map(|i| chars[i]);
        let next = chars.get(i + 1);
        let boundary = c.is_uppercase()
            && previous.is_some_and(|p| {
                p.is_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase()))
            });
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Uppercases the first letter of `word` and lowercases the rest.
fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect()
    })
}

pub fn expand(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new(
            input.span(),
            "`Tags` can only be derived for enums",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "`Tags` cannot be derived for generic enums",
        ));
    }
    if data.variants.is_empty() {
        return Err(Error::new(
            input.span(),
            "`Tags` needs at least one variant",
        ));
    }

    let case = match tag_arg(&input.attrs, "rename_all")? {
        Some(value) => Case::parse(&value)?,
        None => Case::Snake,
    };

    let mut seen = HashMap::new();
    let mut variants = Vec::with_capacity(data.variants.len());
    let mut names = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new(
                variant.span(),
                "`Tags` variants cannot have fields",
            ));
        }
        let name = match tag_arg(&variant.attrs, "rename")? {
            Some(value) => value.value(),
            None => case.apply(&variant.ident.to_string()),
        };
        if name.is_empty() {
            return Err(Error::new(variant.span(), "a tag cannot be empty"));
        }
        if let Some(other) = seen.insert(name.clone(), &variant.ident) {
            let message = format!("`{other}` and `{}` are both tagged `{name}`", variant.ident);
            return Err(Error::new(variant.span(), message));
        }
        variants.push(&variant.ident);
        names.push(name);
    }

    let ident = &input.ident;
    Ok(quote! {
        impl ::core::convert::From<#ident> for ::spire::request::Tag {
            fn from(tag: #ident) -> Self {
                ::spire::request::Tag::from_static(match tag {
                    #(#ident::#variants => #names,)*
                })
            }
        }

        impl ::spire::request::Tags for #ident {
            const ALL: &'static [Self] = &[#(Self::#variants),*];

            fn from_tag(tag: &::spire::request::Tag) -> ::core::option::Option<Self> {
                match tag.as_str() {
                    #(#names => ::core::option::Option::Some(Self::#variants),)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        /// Extracts the request's tag, rejecting untagged requests and tags
        /// of no variant.
        impl<__C, __S> ::spire::extract::FromContext<__C, __S> for #ident
        where
            __C: ::core::marker::Sync,
            __S: ::core::marker::Sync,
        {
            type Rejection = ::spire::Error;

            fn from_context(
                cx: &::spire::Context<__C>,
                _: &__S,
            ) -> impl ::core::future::Future<Output = ::spire::Result<Self>> + ::core::marker::Send
            {
                ::spire::__private::ready(::spire::__private::tag_of(cx))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("ItemReviews"), ["Item", "Reviews"]);
        assert_eq!(words("HTTPPage"), ["HTTP", "Page"]);
        assert_eq!(words("Page2Item"), ["Page2", "Item"]);
        assert_eq!(words("List"), ["List"]);
    }

    #[test]
    fn applies_cases() {
        let cases = [
            (Case::Lower, "itemreviews"),
            (Case::Upper, "ITEMREVIEWS"),
            (Case::Pascal, "ItemReviews"),
            (Case::Camel, "itemReviews"),
            (Case::Snake, "item_reviews"),
            (Case::ScreamingSnake, "ITEM_REVIEWS"),
            (Case::Kebab, "item-reviews"),
            (Case::ScreamingKebab, "ITEM-REVIEWS"),
        ];
        for (case, expected) in cases {
            assert_eq!(case.apply("ItemReviews"), expected);
        }
    }
}
