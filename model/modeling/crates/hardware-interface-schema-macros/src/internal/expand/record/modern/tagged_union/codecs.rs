use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::{codec, scalar, tag_info, tagged_union_geometry};
use crate::internal::syntax::{ModernRecordDecl, TaggedUnionDecl};

pub(in crate::internal::expand::record::modern) fn expand(
    record: &ModernRecordDecl,
    field: &syn::Ident,
    decl: &TaggedUnionDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let name = scalar::type_name(&record.name, field, "Values");
    let raw = scalar::type_name(&record.name, field, "Raw");
    let geometry = tagged_union_geometry(decl, facade)
        .ok_or_else(|| syn::Error::new(field.span(), "union has no storage"))?;
    let extent = &geometry.extent;
    let alignment = &geometry.storage.width;
    let tag = tag_info(record, &decl.tag, &quote!(value))?.ty;
    let owner = &record.name;
    let identity = format_ident!("{}_IDENTITY", field.to_string().to_ascii_uppercase());
    let mut declarations = Vec::new();
    let mut decode = Vec::new();
    let mut encode = Vec::new();
    let mut tags = Vec::new();
    for case in &decl.cases {
        let variant = &case.name;
        let path = case
            .codec
            .as_ref()
            .ok_or_else(|| syn::Error::new(variant.span(), "expected codec case"))?;
        declarations.push(quote!(#[doc = "Generated codec union case."] #variant(<#path as #facade::SchemaCodec>::Value)));
        tags.push(quote!(Self::#variant(_) => #tag::#variant));
        decode.push(quote!(#tag::#variant => {
            let extent = <#path as #facade::SchemaCodec>::EXTENT;
            let child = storage.get(..extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: extent, actual: storage.len() })?;
            let value = <#path as #facade::SchemaCodec>::decode(child)
                .map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(base)))?;
            for (index, byte) in storage.iter().enumerate().skip(<#path as #facade::SchemaCodec>::EXTENT) {
                if *byte != 0 {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                        location: #owner::#identity.at(base.saturating_add(index)), observed: u64::from(*byte),
                    });
                }
            }
            ::core::result::Result::Ok(Self::#variant(value))
        }));
        encode.push(quote!(Self::#variant(value) => {
            let extent = <#path as #facade::SchemaCodec>::EXTENT;
            let actual = storage.len();
            let child = storage.get_mut(..extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: extent, actual })?;
            <#path as #facade::SchemaCodec>::encode(child, value)
                .map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(base)))?;
        }));
    }
    let first = decl
        .cases
        .first()
        .ok_or_else(|| syn::Error::new(field.span(), "union requires a case"))?;
    let first_case = &first.name;
    let first_type = first
        .codec
        .as_ref()
        .ok_or_else(|| syn::Error::new(field.span(), "expected codec case"))?;
    let codec = codec::expand(codec::Context {
        visibility,
        values: &name,
        raw: &raw,
        tag: &tag,
        first_case,
        extent,
        facade,
    });
    Ok(quote! {
        #[doc = "Generated union of schema codec values."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#declarations),* }
        impl #name {
            #[doc = "First declared record case."]
            pub const ZERO: Self = Self::#first_case(<#first_type as #facade::SchemaCodec>::ZERO);
            #[doc = "Aligned storage size shared by all cases."]
            pub const RECORD_SIZE: usize = #extent;
            #[doc = "Opaque union storage; case records describe the status-specific fields."]
            pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(stringify!(#name), Self::RECORD_SIZE, #alignment, &[], &[]);
            #[doc = "Returns the selected case discriminator."]
            pub const fn tag(&self) -> #tag { match self { #(#tags),* } }
            fn decode(bytes: &[u8], base: usize, tag: #tag) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let end = base.checked_add(Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
                let storage = bytes.get(base..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                match tag { #(#decode),* }
            }
            fn encode_into(&self, bytes: &mut [u8], base: usize) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let end = base.checked_add(Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual })?;
                let storage = bytes.get_mut(base..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                storage.fill(0);
                match self { #(#encode),* }
                ::core::result::Result::Ok(())
            }
        }
        impl ::core::default::Default for #name { fn default() -> Self { Self::ZERO } }
        #codec
    })
}
