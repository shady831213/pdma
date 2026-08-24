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
    let raw_type = scalar::ty(decl.storage);
    let raw_maximum = decl.storage.maximum();
    let declarations = decl.fields.iter().map(|field| {
        let field_name = &field.name;
        let width = field.end.0 - field.start.0;
        if width == 1 {
            quote!(#[doc = "Generated one-bit boolean field."] #visibility #field_name: bool)
        } else {
            let ty = scalar::ty(scalar::for_bits(width));
            quote!(#[doc = "Generated packed integer field."] #visibility #field_name: #ty)
        }
    });
    let zero_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        if field.end.0 - field.start.0 == 1 {
            quote!(#name: false)
        } else {
            quote!(#name: 0)
        }
    });
    let decoded_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let start = field.start.0;
        let width = field.end.0 - start;
        let mask = mask(width);
        if width == 1 {
            quote!(#name: ((observed >> #start) & 1) != 0)
        } else {
            let scalar = scalar::for_bits(width);
            let narrowed = scalar::from_u64(scalar, quote!((observed >> #start) & #mask));
            quote!(#name: #narrowed)
        }
    });
    let encoded_fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let start = field.start.0;
        let width = field.end.0 - start;
        let maximum = mask(width);
        if width == 1 {
            quote!(if self.#name { observed |= 1_u64 << #start; })
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
                observed |= value << #start;
            })
        }
    });
    let used_mask = decl.fields.iter().fold(0_u64, |mask, field| {
        mask | (self::mask(field.end.0 - field.start.0) << field.start.0)
    });
    let reserved_mask = raw_maximum & !used_mask;
    let narrowed_raw = scalar::from_u64(decl.storage, quote!(observed));
    quote! {
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
    }
}

const fn mask(width: usize) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}
