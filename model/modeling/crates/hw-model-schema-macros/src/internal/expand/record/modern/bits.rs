use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use crate::internal::syntax::BitsDecl;

use super::scalar;

pub(super) fn expand(
    record: &syn::Ident,
    field: &syn::Ident,
    decl: &BitsDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let name = scalar::type_name(record, field, "Values");
    let raw_name = scalar::type_name(record, field, "Raw");
    let raw_type = scalar::ty(decl.storage);
    let raw_maximum = decl.storage.maximum();
    let declarations = decl.fields.iter().map(|field| {
        let field_name = &field.name;
        let rust_docs = field.documentation.rust_docs();
        let ty = crate::internal::expand::bits::field_type(&name, field);
        quote!(#rust_docs #[doc = "Generated packed field."] #visibility #field_name: #ty)
    });
    let zero_fields = decl.fields.iter().map(|field| {
        let member = &field.name;
        let value = crate::internal::expand::bits::field_zero(&name, field);
        quote!(#member: #value)
    });
    let decoded_fields = decl.fields.iter().map(|field| {
        let member = &field.name;
        let observed = quote!(observed);
        let value =
            crate::internal::expand::bits::field_decode(&name, field, &observed, &quote!(base));
        quote!(#member: #value)
    });
    let encoded_fields = encoded_fields(decl, facade);
    let used_mask = crate::internal::validate::bits::declared_mask(&decl.fields);
    let reserved_mask = raw_maximum & !used_mask;
    let narrowed_raw = scalar::from_u64(decl.storage, quote!(observed));
    let codec = codec(&name, &raw_name, facade);
    let member_identities = member_identities(decl, &name, facade);
    let enums = crate::internal::expand::bits::enum_items(&name, &decl.fields, visibility, facade)?;
    Ok(quote! {
        #enums
        #[doc = "Typed raw scalar for the generated packed-bit field."]
        #[derive(Clone, Copy, Debug, ::core::default::Default, Eq, PartialEq)]
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
            #[doc = "Stable identity for reserved bits outside declared members."]
            pub const RESERVED_IDENTITY: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(stringify!(#name), "reserved");
            #member_identities
            #[doc = "Semantic default with zero integers and first-declared enum variants."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            #[doc = "Parses packed bits and rejects nonzero reserved ranges."]
            pub fn from_raw(raw: #raw_type, offset: usize) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                Self::from_raw_at(raw, #facade::SchemaCoordinate::Byte(offset))
            }
            fn from_raw_at(raw: #raw_type, base: #facade::SchemaCoordinate) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let observed = u64::from(raw);
                let reserved = observed & #reserved_mask;
                if reserved != 0 {
                    let mut first_reserved = 0_usize;
                    while reserved & (1_u64 << first_reserved) == 0 {
                        first_reserved += 1;
                    }
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                        location: Self::RESERVED_IDENTITY.at_bit(first_reserved),
                        observed: reserved,
                    }.rebased(base));
                }
                ::core::result::Result::Ok(Self { #(#decoded_fields,)* })
            }
            #[doc = "Packs typed fields into their raw integer storage."]
            pub fn raw(self, offset: usize) -> ::core::result::Result<#raw_type, #facade::SchemaCodecError> {
                self.raw_at(#facade::SchemaCoordinate::Byte(offset))
            }
            fn raw_at(self, base: #facade::SchemaCoordinate) -> ::core::result::Result<#raw_type, #facade::SchemaCodecError> {
                let mut observed = 0_u64;
                #encoded_fields
                ::core::result::Result::Ok(#narrowed_raw)
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        #codec
    })
}

fn encoded_fields(decl: &BitsDecl, facade: &TokenStream) -> TokenStream {
    let fields = decl.fields.iter().map(|field| {
        let name = &field.name;
        let identity = format_ident!("{}_IDENTITY", name.unraw().to_string().to_ascii_uppercase());
        let lsb = field.start.0;
        let width = crate::internal::expand::bits::width(field);
        let maximum = crate::internal::validate::bits::bit_mask(width);
        let value = quote!(self.#name);
        let encoded = crate::internal::expand::bits::encode(field, &value);
        let raw = crate::internal::expand::bits::field_raw(field, &value);
        if width == 1 {
            quote!(observed |= #encoded;)
        } else {
            quote!({
                let value = #raw;
                if value > #maximum {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NarrowingOverflow {
                        location: Self::#identity.at_bit(#lsb),
                        observed: value,
                        maximum: #maximum,
                    }.rebased(base));
                }
                observed |= #encoded;
            })
        }
    });
    quote!(#(#fields)*)
}

fn member_identities(decl: &BitsDecl, owner: &syn::Ident, facade: &TokenStream) -> TokenStream {
    let identities = decl.fields.iter().map(|field| {
        let identity = format_ident!(
            "{}_IDENTITY",
            field.name.unraw().to_string().to_ascii_uppercase()
        );
        let member = field.name.to_string();
        quote! {
            #[doc = "Stable identity for one declared bit member."]
            pub const #identity: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(stringify!(#owner), #member);
        }
    });
    quote!(#(#identities)*)
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
