use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{TaggedUnionFieldDecl, TaggedUnionFieldType};

use super::super::scalar;

pub(super) struct Context<'a> {
    pub(super) owner: &'a syn::Ident,
    pub(super) field: &'a TaggedUnionFieldDecl,
    pub(super) relative: TokenStream,
    pub(super) quantum: &'a TokenStream,
    pub(super) facade: &'a TokenStream,
    pub(super) endian: &'a syn::Ident,
}

pub(super) fn decode(context: Context<'_>) -> TokenStream {
    let Context {
        owner,
        field,
        relative,
        quantum,
        facade,
        endian,
    } = context;
    let name = &field.name;
    let identity = format_ident!("{}_IDENTITY", name.to_string().to_ascii_uppercase());
    let decoded = match &field.ty {
        TaggedUnionFieldType::Scalar(scalar_type) => {
            let ty = scalar::ty(*scalar_type);
            quote!(#facade::read_scalar::<#ty>(bytes, #facade::Endian::#endian))
        }
        TaggedUnionFieldType::Bits(path) => quote!(
            <#path as #facade::BitsLayout>::decode_record(
                bytes,
                #facade::Endian::#endian,
                #owner::#identity.at(offset),
            )
        ),
    };
    quote!(#name: {
        let offset = base.checked_add(#relative).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let end = offset.checked_add(#quantum).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let bytes = bytes.get(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
        #decoded?
    })
}

pub(super) fn encode(context: Context<'_>) -> TokenStream {
    let Context {
        owner,
        field,
        relative,
        quantum,
        facade,
        endian,
        ..
    } = context;
    let name = &field.name;
    let identity = format_ident!("{}_IDENTITY", name.to_string().to_ascii_uppercase());
    let binding = binding(name);
    let encoded = match &field.ty {
        TaggedUnionFieldType::Scalar(_) => {
            quote!(#facade::write_scalar(bytes, #facade::Endian::#endian, *#binding))
        }
        TaggedUnionFieldType::Bits(path) => quote!(
            <#path as #facade::BitsLayout>::encode_record(
                bytes,
                #facade::Endian::#endian,
                #binding,
                #owner::#identity.at(offset),
            )
        ),
    };
    quote!({
        let offset = base.checked_add(#relative).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let end = offset.checked_add(#quantum).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let actual = bytes.len();
        let bytes = bytes.get_mut(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
        #encoded?;
    })
}

pub(super) fn binding(name: &syn::Ident) -> syn::Ident {
    format_ident!(
        "__schema_union_value_{}",
        name,
        span = proc_macro2::Span::mixed_site()
    )
}

pub(super) fn semantic_type(ty: &TaggedUnionFieldType, facade: &TokenStream) -> TokenStream {
    match ty {
        TaggedUnionFieldType::Scalar(scalar_type) => scalar::ty(*scalar_type),
        TaggedUnionFieldType::Bits(path) => quote!(<#path as #facade::BitsLayout>::Value),
    }
}
