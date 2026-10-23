//! `#[derive(Revisit)]`.

use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, LitStr, Result};

/// Parses an interval such as `90s`, `30m`, `1h30m` or `1d` into seconds.
fn seconds(value: &LitStr) -> Result<u64> {
    let invalid = || {
        Error::new(
            value.span(),
            "expected an interval such as \"90s\", \"30m\", \"1h30m\" or \"1d\"",
        )
    };
    let text = value.value();
    let mut total: u64 = 0;
    let mut number = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c {
            's' => 1,
            'm' => 60,
            'h' => 60 * 60,
            'd' => 24 * 60 * 60,
            _ => return Err(invalid()),
        };
        let amount: u64 = number.parse().map_err(|_| invalid())?;
        number.clear();
        total = amount
            .checked_mul(unit)
            .and_then(|part| total.checked_add(part))
            .ok_or_else(invalid)?;
    }
    if !number.is_empty() || text.is_empty() {
        return Err(invalid());
    }
    if total == 0 {
        return Err(Error::new(
            value.span(),
            "a revisit interval must be above zero",
        ));
    }
    Ok(total)
}

pub fn expand(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new(
            input.span(),
            "`Revisit` can only be derived for enums",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "`Revisit` cannot be derived for generic enums",
        ));
    }

    let mut arms = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new(
                variant.span(),
                "`Revisit` variants cannot have fields",
            ));
        }
        let mut interval = None;
        for attr in variant
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("revisit"))
        {
            if interval.is_some() {
                return Err(Error::new(attr.span(), "duplicate `#[revisit]` attribute"));
            }
            interval = Some(seconds(&attr.parse_args::<LitStr>()?)?);
        }
        let ident = &variant.ident;
        let value = interval.map_or_else(
            || quote!(::core::option::Option::None),
            |seconds| quote!(::core::option::Option::Some(::core::time::Duration::from_secs(#seconds))),
        );
        arms.push(quote!(Self::#ident => #value));
    }

    let ident = &input.ident;
    Ok(quote! {
        impl ::spire::request::Revisit for #ident {
            fn interval(self) -> ::core::option::Option<::core::time::Duration> {
                match self {
                    #(#arms,)*
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use proc_macro2::Span;

    use super::*;

    fn parse(text: &str) -> Result<u64> {
        seconds(&LitStr::new(text, Span::call_site()))
    }

    #[test]
    fn parses_intervals() {
        assert_eq!(parse("90s").unwrap(), 90);
        assert_eq!(parse("30m").unwrap(), 1800);
        assert_eq!(parse("1h30m").unwrap(), 5400);
        assert_eq!(parse("2d").unwrap(), 172_800);
    }

    #[test]
    fn rejects_malformed_and_zero_intervals() {
        for text in ["", "1", "h", "1hr", "1.5h", "-1h", "0s", "0h0m"] {
            assert!(parse(text).is_err(), "{text:?} should be rejected");
        }
        assert!(parse("99999999999999999999d").is_err(), "overflow");
    }
}
