use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernTypeDecl, ScopedPath};

use super::super::{layout::PlacedField, scalar};

pub(super) fn decode(
    marker: &syn::Ident,
    field: &PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> syn::Result<TokenStream> {
    let name = &field.decl.name;
    let offset = &field.offset_name;
    let identity = format_ident!("{}_IDENTITY", name.to_string().to_ascii_uppercase());
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, constant } => {
            let ty = scalar::ty(*scalar);
            let width = scalar.width();
            let end = quote!(#marker::#offset.saturating_add(#width));
            Ok(constant.as_ref().map_or_else(
                || quote!(let #name: #ty = {
                    let end = #end;
                    let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                    #facade::read_scalar(field, #facade::Endian::#endian)?
                };),
                |path| {
                    let expected_path = scalar::record_size_path(path);
                    let expected_tokens = quote!(#expected_path);
                    let expected = scalar::from_usize(*scalar, &expected_tokens);
                    quote!({
                        let end = #end;
                        let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                        let observed: #ty = #facade::read_scalar(field, #facade::Endian::#endian)?;
                        let expected: #ty = #expected;
                        if observed != expected {
                            return ::core::result::Result::Err(#facade::SchemaCodecError::ConstantMismatch {
                                location: #marker::#identity.at(#marker::#offset),
                                observed: u64::from(observed),
                                expected: u64::from(expected),
                            });
                        }
                    })
                },
            ))
        }
        ModernTypeDecl::Enum(decl) => {
            let ty = scalar::type_name(marker, name, "");
            let raw = scalar::ty(decl.storage);
            let width = decl.storage.width();
            Ok(quote!(let #name = {
                let end = #marker::#offset.saturating_add(#width);
                let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                let raw: #raw = #facade::read_scalar(field, #facade::Endian::#endian)?;
                #ty::from_raw_at(raw, #marker::#identity.at(#marker::#offset))?
            };))
        }
        ModernTypeDecl::Bits(decl) => {
            let ty = scalar::type_name(marker, name, "Values");
            let raw = scalar::ty(decl.storage);
            let width = decl.storage.width();
            Ok(quote!(let #name = {
                let end = #marker::#offset.saturating_add(#width);
                let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                let raw: #raw = #facade::read_scalar(field, #facade::Endian::#endian)?;
                #ty::from_raw(raw, #marker::#offset)?
            };))
        }
        ModernTypeDecl::BitsReference(path) => Ok(quote!(let #name = {
            let end = #marker::#offset.saturating_add(#path::EXTENT);
            let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
            <#path as #facade::BitsLayout>::decode_record(
                field,
                #facade::Endian::#endian,
                #marker::#identity.at(#marker::#offset),
            )?
        };)),
        ModernTypeDecl::Record(path) => Ok(quote!(let #name = {
            let end = #marker::#offset.checked_add(#path::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
            let field = bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
            <#path as #facade::RecordLayout>::decode(field)
                .map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(#marker::#offset)))?
        };)),
        ModernTypeDecl::Union(decl) => {
            let ty = scalar::type_name(marker, name, "Values");
            let tag = local_tag(&decl.tag)?;
            Ok(quote!(let #name = #ty::decode(bytes, #marker::#offset, #tag)?;))
        }
    }
}

pub(super) fn encode(
    marker: &syn::Ident,
    field: &PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let name = &field.decl.name;
    let offset = &field.offset_name;
    let identity = format_ident!("{}_IDENTITY", name.to_string().to_ascii_uppercase());
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, constant } => {
            let width = scalar.width();
            let value = constant.as_ref().map_or_else(
                || quote!(normalized.#name),
                |path| {
                    let path = scalar::record_size_path(path);
                    let tokens = quote!(#path);
                    scalar::from_usize(*scalar, &tokens)
                },
            );
            quote!({
                let end = #marker::#offset.saturating_add(#width);
                let actual = encoded.len();
                let field = encoded.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                #facade::write_scalar(field, #facade::Endian::#endian, #value)?;
            })
        }
        ModernTypeDecl::Enum(decl) => {
            let width = decl.storage.width();
            quote!({
                let end = #marker::#offset.saturating_add(#width);
                let actual = encoded.len();
                let field = encoded.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                #facade::write_scalar(field, #facade::Endian::#endian, normalized.#name.raw())?;
            })
        }
        ModernTypeDecl::Bits(decl) => {
            let width = decl.storage.width();
            quote!({
                let end = #marker::#offset.saturating_add(#width);
                let raw = normalized.#name.raw(#marker::#offset)?;
                let actual = encoded.len();
                let field = encoded.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                #facade::write_scalar(field, #facade::Endian::#endian, raw)?;
            })
        }
        ModernTypeDecl::BitsReference(path) => quote!({
            let end = #marker::#offset.saturating_add(#path::EXTENT);
            let actual = encoded.len();
            let field = encoded.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
            <#path as #facade::BitsLayout>::encode_record(
                field,
                #facade::Endian::#endian,
                &normalized.#name,
                #marker::#identity.at(#marker::#offset),
            )?;
        }),
        ModernTypeDecl::Record(path) => quote!({
            let end = #marker::#offset.checked_add(#path::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: encoded.len() })?;
            let actual = encoded.len();
            let field = encoded.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
            <#path as #facade::RecordLayout>::encode(field, &normalized.#name)
                .map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(#marker::#offset)))?;
        }),
        ModernTypeDecl::Union(_) => {
            quote!(normalized.#name.encode_into(&mut encoded, #marker::#offset)?;)
        }
    }
}

fn local_tag(path: &ScopedPath) -> syn::Result<TokenStream> {
    let Some(first) = path.segments.first() else {
        return Err(syn::Error::new(path.span, "tag path is empty"));
    };
    path.segments
        .get(1)
        .map_or_else(|| Ok(quote!(#first)), |second| Ok(quote!(#first.#second)))
}
