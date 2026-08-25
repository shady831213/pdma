use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::syntax::BitsDecl;

use super::scalar;

pub(super) fn expand(
    record: &syn::Ident,
    field: &syn::Ident,
    decl: &BitsDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> TokenStream {
    let name = scalar::type_name(record, field, "Values");
    let raw_name = scalar::type_name(record, field, "Raw");
    let raw_type = scalar::ty(decl.storage);
    let raw_maximum = decl.storage.maximum();
    let declarations = decl.fields.iter().map(|field| {
        let field_name = &field.name;
        let width = crate::internal::expand::bits::width(field);
        if width == 1 {
            quote!(#[doc = "Generated one-bit boolean field."] #visibility #field_name: bool)
        } else {
            let ty = crate::internal::expand::bits::semantic_type(field);
            quote!(#[doc = "Generated packed integer field."] #visibility #field_name: #ty)
        }
    });
    let zero_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let value = crate::internal::expand::bits::zero(field);
        quote!(#name: #value)
    });
    let decoded_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let observed = quote!(observed);
        let value = crate::internal::expand::bits::decode(field, &observed);
        quote!(#name: #value)
    });
    let encoded_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let width = crate::internal::expand::bits::width(field);
        let maximum = crate::internal::validate::bits::bit_mask(width);
        let value = quote!(self.#name);
        let encoded = crate::internal::expand::bits::encode(field, &value);
        if width == 1 {
            quote!(observed |= #encoded;)
        } else {
            quote!({
                let value = u64::from(self.#name);
                if value > #maximum {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NarrowingOverflow {
                        offset,
                        observed: value,
                        maximum: #maximum,
                    });
                }
                observed |= #encoded;
            })
        }
    });
    let used_mask = crate::internal::validate::bits::declared_mask(&decl.fields);
    let reserved_mask = raw_maximum & !used_mask;
    let narrowed_raw = scalar::from_u64(decl.storage, quote!(observed));
    let codec = codec(&name, &raw_name, facade);
    quote! {
        #[doc = "Typed raw scalar for the generated packed-bit field."]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        #visibility struct #raw_name(#raw_type);
        impl #raw_name {
            #[doc = "Creates one typed raw scalar."]
            pub const fn new(value: #raw_type) -> Self { Self(value) }
            #[doc = "Returns the raw scalar."]
            pub const fn value(self) -> #raw_type { self.0 }
        }
        #[doc = "Generated typed packed-bit record value."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #name { #(#declarations,)* }
        impl #name {
            #[doc = "All-zero packed-bit value."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            #[doc = "Parses packed bits and rejects nonzero reserved ranges."]
            pub fn from_raw(raw: #raw_type, offset: usize) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let observed = u64::from(raw);
                let reserved = observed & #reserved_mask;
                if reserved != 0 {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved { offset, observed: reserved });
                }
                ::core::result::Result::Ok(Self { #(#decoded_fields,)* })
            }
            #[doc = "Packs typed fields into their raw integer storage."]
            pub fn raw(self, offset: usize) -> ::core::result::Result<#raw_type, #facade::SchemaCodecError> {
                let mut observed = 0_u64;
                #(#encoded_fields)*
                ::core::result::Result::Ok(#narrowed_raw)
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        #codec
    }
}

fn codec(values: &syn::Ident, raw: &syn::Ident, facade: &TokenStream) -> TokenStream {
    quote! {
        impl #facade::Decode for #raw {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #values::from_raw(self.0, 0)
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                self.raw(0).map(#raw::new)
            }
        }
        impl ::core::convert::TryFrom<#raw> for #values {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw) -> ::core::result::Result<Self, Self::Error> {
                <#raw as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#values> for #raw {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #values) -> ::core::result::Result<Self, Self::Error> {
                <#values as #facade::Encode>::encode(value)
            }
        }
    }
}
