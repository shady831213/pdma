use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::UnionFieldDecl;

use super::super::scalar;

#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub(super) owner: &'a syn::Ident,
    pub(super) field: &'a UnionFieldDecl,
    pub(super) relative: usize,
    pub(super) quantum: usize,
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
    let storage_type = scalar::ty(field.storage);
    let maximum = field.scalar.maximum();
    let narrowed = scalar::from_u64(field.scalar, quote!(observed));
    quote!(#name: {
        let offset = base.checked_add(#relative).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let end = offset.checked_add(#quantum).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let field = bytes.get(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
        let stored: #storage_type = #facade::read_scalar(field, #facade::Endian::#endian)?;
        let observed = u64::from(stored);
        if observed > #maximum {
            return ::core::result::Result::Err(#facade::SchemaCodecError::NarrowingOverflow {
                location: #owner::#identity.at(offset),
                observed,
                maximum: #maximum,
            });
        }
        #narrowed
    })
}

pub(super) fn encode(context: Context<'_>) -> TokenStream {
    let Context {
        field,
        relative,
        quantum,
        facade,
        endian,
        ..
    } = context;
    let name = &field.name;
    let stored = scalar::from_u64(field.storage, quote!(u64::from(*#name)));
    quote!({
        let offset = base.checked_add(#relative).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let end = offset.checked_add(#quantum).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let actual = bytes.len();
        let field = bytes.get_mut(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
        #facade::write_scalar(field, #facade::Endian::#endian, #stored)?;
    })
}
