use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, LitStr, Result, Token, parse::ParseStream};

#[derive(Default)]
pub(in crate::internal) struct Documentation {
    short_description: Option<LitStr>,
    description: Option<LitStr>,
}

impl Documentation {
    pub(in crate::internal) fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut documentation = Self::default();
        while input.peek(Token![#]) {
            let attributes = input.call(Attribute::parse_outer)?;
            for attribute in attributes {
                let target = if attribute.path().is_ident("short_description") {
                    &mut documentation.short_description
                } else if attribute.path().is_ident("description") {
                    &mut documentation.description
                } else {
                    return Err(syn::Error::new_spanned(
                        attribute.path(),
                        "unsupported schema attribute; expected short_description or description",
                    ));
                };
                if target.is_some() {
                    return Err(syn::Error::new_spanned(
                        attribute.path(),
                        "duplicate documentation attribute",
                    ));
                }
                let value = &attribute.meta.require_name_value()?.value;
                let literal = syn::parse2::<LitStr>(quote!(#value)).map_err(|_| {
                    syn::Error::new_spanned(
                        value,
                        "documentation attribute must be a string literal",
                    )
                })?;
                *target = Some(literal);
            }
        }
        Ok(documentation)
    }

    pub(in crate::internal) fn metadata(&self, facade: &TokenStream) -> TokenStream {
        let optional = |value: &Option<LitStr>| {
            value.as_ref().map_or_else(
                || quote!(::core::option::Option::None),
                |value| quote!(::core::option::Option::Some(#value)),
            )
        };
        let short = optional(&self.short_description);
        let long = optional(&self.description);
        quote!(#facade::Documentation { short_description: #short, description: #long })
    }

    pub(in crate::internal) fn rust_docs(&self) -> TokenStream {
        let short = self
            .short_description
            .iter()
            .map(|value| quote!(#[doc = #value]));
        let separator = (self.short_description.is_some() && self.description.is_some())
            .then(|| quote!(#[doc = ""]));
        let long = self.description.iter().map(|value| quote!(#[doc = #value]));
        quote!(#(#short)* #separator #(#long)*)
    }
}
