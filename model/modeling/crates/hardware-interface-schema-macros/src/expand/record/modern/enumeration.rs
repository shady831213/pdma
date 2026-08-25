use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::{syntax::EnumDecl, validate::record::modern::enum_variants};

use super::scalar;

pub(super) fn expand(
    record: &syn::Ident,
    field: &syn::Ident,
    decl: &EnumDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let name = scalar::type_name(record, field, "");
    let raw_name = scalar::type_name(record, field, "Raw");
    let raw_type = scalar::ty(decl.storage);
    let variants = enum_variants(decl)?;
    let declarations = variants
        .iter()
        .map(|(variant, _)| quote!(#[doc = "Generated enum variant."] #variant));
    let from_arms = variants
        .iter()
        .map(|(variant, value)| quote!(#value => ::core::result::Result::Ok(Self::#variant),));
    let raw_arms = variants.iter().map(|(variant, value)| {
        let value = scalar::from_u64(decl.storage, quote!(#value));
        quote!(Self::#variant => #value,)
    });
    let Some((first, _)) = variants.first() else {
        return Err(syn::Error::new(field.span(), "enum requires a variant"));
    };
    Ok(quote! {
        #[doc = "Typed raw discriminant for the generated record enum."]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        #visibility struct #raw_name(#raw_type);
        impl #raw_name {
            #[doc = "Creates one typed raw discriminant."]
            pub const fn new(value: #raw_type) -> Self { Self(value) }
            #[doc = "Returns the raw discriminant."]
            pub const fn value(self) -> #raw_type { self.0 }
        }
        #[doc = "Generated typed record enum."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#declarations),* }
        impl #name {
            #[doc = "First declared value used by generated zero records."]
            pub const ZERO: Self = Self::#first;
            #[doc = "Parses one raw discriminant."]
            pub fn from_raw(raw: #raw_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                match u64::from(raw) {
                    #(#from_arms)*
                    observed => ::core::result::Result::Err(#facade::SchemaCodecError::UnknownTag { observed }),
                }
            }
            #[doc = "Returns the assigned raw discriminant."]
            pub const fn raw(self) -> #raw_type {
                match self { #(#raw_arms)* }
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        impl #facade::Decode for #raw_name {
            type Decoded = #name;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #name::from_raw(self.0)
            }
        }
        impl #facade::Encode for #name {
            type Encoded = #raw_name;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                ::core::result::Result::Ok(#raw_name::new(self.raw()))
            }
        }
        impl ::core::convert::TryFrom<#raw_name> for #name {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw_name) -> ::core::result::Result<Self, Self::Error> {
                <#raw_name as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#name> for #raw_name {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #name) -> ::core::result::Result<Self, Self::Error> {
                <#name as #facade::Encode>::encode(value)
            }
        }
    })
}
