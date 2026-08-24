use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl, ScalarType};

use super::{layout::RecordPlan, layout::union_geometry, scalar};

pub(super) fn expand(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let visibility = &decl.visibility;
    let values = format_ident!("{marker}Values");
    let view = format_ident!("{marker}View");
    let view_mut = format_ident!("{marker}ViewMut");
    let immutable_getters = plan
        .fields
        .iter()
        .flat_map(|field| getters(marker, field, facade, &decl.endian));
    let mutable_getters = plan
        .fields
        .iter()
        .flat_map(|field| getters(marker, field, facade, &decl.endian));
    let setters = plan
        .fields
        .iter()
        .flat_map(|field| setters(marker, field, facade, &decl.endian));
    quote! {
        #[doc = "Checked immutable view over one inferred record."]
        #[derive(Debug, Eq, PartialEq)]
        #visibility struct #view<'a> { bytes: &'a [u8] }
        impl<'a> #view<'a> {
            #[doc = "Checks the inferred extent and creates a view."]
            pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..#marker::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #marker::RECORD_SIZE, actual })?;
                ::core::result::Result::Ok(Self { bytes })
            }
            #(#immutable_getters)*
            #[doc = "Decodes the complete semantic value."]
            pub fn values(&self) -> ::core::result::Result<#values, #facade::SchemaCodecError> {
                #values::from_bytes(self.bytes)
            }
        }
        #[doc = "Checked mutable view over one inferred record."]
        #[derive(Debug)]
        #visibility struct #view_mut<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut<'a> {
            #[doc = "Checks the inferred extent and creates a mutable view."]
            pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get_mut(..#marker::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #marker::RECORD_SIZE, actual })?;
                ::core::result::Result::Ok(Self { bytes })
            }
            #(#mutable_getters)*
            #(#setters)*
            #[doc = "Decodes the complete semantic value."]
            pub fn values(&self) -> ::core::result::Result<#values, #facade::SchemaCodecError> {
                #values::from_bytes(self.bytes)
            }
            #[doc = "Atomically replaces the complete semantic value."]
            pub fn set_values(&mut self, value: &#values) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                value.encode_into(self.bytes)
            }
        }
    }
}

fn getters(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> Vec<TokenStream> {
    let name = &field.decl.name;
    let offset = &field.offset_name;
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => {
            vec![scalar_getter_at(
                marker,
                name,
                offset,
                (0, *scalar),
                facade,
                endian,
            )]
        }
        ModernTypeDecl::Enum(decl) => vec![scalar_getter_at(
            marker,
            name,
            offset,
            (0, decl.storage),
            facade,
            endian,
        )],
        ModernTypeDecl::Bits(decl) => vec![scalar_getter_at(
            marker,
            name,
            offset,
            (0, decl.storage),
            facade,
            endian,
        )],
        ModernTypeDecl::Record(path) => vec![quote! {
            #[doc = "Creates a checked immutable view of the nested record."]
            pub fn #name(&self) -> ::core::result::Result<<#path as #facade::RecordLayout>::View<'_>, #facade::SchemaCodecError> {
                let end = #marker::#offset.saturating_add(#path::RECORD_SIZE);
                let bytes = self.bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
                <#path as #facade::RecordLayout>::view(bytes)
            }
        }],
        ModernTypeDecl::Union(union) => union_geometry(union).map_or_else(Vec::new, |geometry| {
            (0..geometry.slots)
                .map(|index| {
                    let method = format_ident!("word{index}");
                    let relative = index * geometry.storage.width();
                    scalar_getter_at(
                        marker,
                        &method,
                        offset,
                        (relative, geometry.storage),
                        facade,
                        endian,
                    )
                })
                .collect()
        }),
    }
}

fn setters(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> Vec<TokenStream> {
    let name = &field.decl.name;
    let offset = &field.offset_name;
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => {
            vec![scalar_setter_at(
                marker,
                name,
                offset,
                (0, *scalar),
                facade,
                endian,
            )]
        }
        ModernTypeDecl::Enum(decl) => vec![scalar_setter_at(
            marker,
            name,
            offset,
            (0, decl.storage),
            facade,
            endian,
        )],
        ModernTypeDecl::Bits(decl) => vec![scalar_setter_at(
            marker,
            name,
            offset,
            (0, decl.storage),
            facade,
            endian,
        )],
        ModernTypeDecl::Record(path) => {
            let method = format_ident!("{name}_mut");
            vec![quote! {
                #[doc = "Creates a checked mutable view of the nested record."]
                pub fn #method(&mut self) -> ::core::result::Result<<#path as #facade::RecordLayout>::ViewMut<'_>, #facade::SchemaCodecError> {
                    let end = #marker::#offset.saturating_add(#path::RECORD_SIZE);
                    let actual = self.bytes.len();
                    let bytes = self.bytes.get_mut(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                    <#path as #facade::RecordLayout>::view_mut(bytes)
                }
            }]
        }
        ModernTypeDecl::Union(union) => union_geometry(union).map_or_else(Vec::new, |geometry| {
            (0..geometry.slots)
                .map(|index| {
                    let method = format_ident!("word{index}");
                    let relative = index * geometry.storage.width();
                    scalar_setter_at(
                        marker,
                        &method,
                        offset,
                        (relative, geometry.storage),
                        facade,
                        endian,
                    )
                })
                .collect()
        }),
    }
}

fn scalar_getter_at(
    marker: &syn::Ident,
    method: &syn::Ident,
    offset: &syn::Ident,
    storage: (usize, ScalarType),
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let (relative, scalar_type) = storage;
    let ty = scalar::ty(scalar_type);
    let width = scalar_type.width();
    quote! {
        #[doc = "Reads one raw scalar storage field."]
        pub fn #method(&self) -> ::core::result::Result<#ty, #facade::SchemaCodecError> {
            let start = #marker::#offset.saturating_add(#relative);
            let end = start.saturating_add(#width);
            let bytes = self.bytes.get(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
            #facade::read_scalar(bytes, #facade::Endian::#endian)
        }
    }
}

fn scalar_setter_at(
    marker: &syn::Ident,
    method: &syn::Ident,
    offset: &syn::Ident,
    storage: (usize, ScalarType),
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let (relative, scalar_type) = storage;
    let setter = format_ident!("set_{method}");
    let ty = scalar::ty(scalar_type);
    let width = scalar_type.width();
    quote! {
        #[doc = "Writes one raw scalar storage field."]
        pub fn #setter(&mut self, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> {
            let start = #marker::#offset.saturating_add(#relative);
            let end = start.saturating_add(#width);
            let actual = self.bytes.len();
            let bytes = self.bytes.get_mut(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
            #facade::write_scalar(bytes, #facade::Endian::#endian, value)
        }
    }
}
