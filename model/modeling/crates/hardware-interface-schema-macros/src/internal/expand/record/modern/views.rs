use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};

use super::{layout::RecordPlan, layout::union_geometry};

mod bits;
mod raw_scalar;

pub(super) fn expand(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let visibility = &decl.visibility;
    let values = marker;
    let view = format_ident!("{marker}View");
    let view_mut = format_ident!("{marker}ViewMut");
    let immutable_getters = plan.fields.iter().flat_map(|field| {
        let rust_docs = field.decl.documentation.rust_docs();
        getters(marker, field, facade, &decl.endian)
            .into_iter()
            .map(move |item| quote!(#rust_docs #item))
    });
    let mutable_getters = plan.fields.iter().flat_map(|field| {
        let rust_docs = field.decl.documentation.rust_docs();
        getters(marker, field, facade, &decl.endian)
            .into_iter()
            .map(move |item| quote!(#rust_docs #item))
    });
    let setters = plan.fields.iter().flat_map(|field| {
        let rust_docs = field.decl.documentation.rust_docs();
        setters(marker, field, facade, &decl.endian)
            .into_iter()
            .map(move |item| quote!(#rust_docs #item))
    });
    quote! {
        #[doc = "Checked immutable view over one inferred record."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        impl<'a> #facade::Decode for #view<'a> {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #values::from_bytes(self.bytes)
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
    let scalar_context = raw_scalar::ViewContext {
        marker,
        offset,
        facade,
        endian,
    };
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => {
            vec![raw_scalar::getter(scalar_context, name, (0, *scalar))]
        }
        ModernTypeDecl::Enum(decl) => {
            vec![raw_scalar::getter(scalar_context, name, (0, decl.storage))]
        }
        ModernTypeDecl::Bits(decl) => {
            vec![raw_scalar::getter(scalar_context, name, (0, decl.storage))]
        }
        ModernTypeDecl::BitsReference(path) => {
            vec![bits::getter(bits::ViewContext {
                marker,
                name,
                offset,
                path,
                facade,
                endian,
            })]
        }
        ModernTypeDecl::Record(path) => vec![quote! {
            #[doc = "Creates a checked immutable view of the nested record."]
            pub fn #name(&self) -> ::core::result::Result<<#path as #facade::RecordLayout>::View<'_>, #facade::SchemaCodecError> {
                let end = #marker::#offset.saturating_add(#path::RECORD_SIZE);
                let bytes = self.bytes.get(#marker::#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
                <#path as #facade::RecordLayout>::view(bytes)
            }
        }],
        ModernTypeDecl::Union(union) => {
            union_geometry(union, facade).map_or_else(Vec::new, |geometry| {
                let width = &geometry.storage.width;
                let ty = &geometry.storage.ty;
                (0..geometry.slots)
                    .map(|index| {
                        let method = if field.scoped_union {
                            format_ident!("{name}_word{index}")
                        } else {
                            format_ident!("word{index}")
                        };
                        let relative = quote!((#index).saturating_mul(#width));
                        raw_scalar::dynamic_getter(scalar_context, &method, &relative, ty, width)
                    })
                    .collect()
            })
        }
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
    let scalar_context = raw_scalar::ViewContext {
        marker,
        offset,
        facade,
        endian,
    };
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => {
            vec![raw_scalar::setter(scalar_context, name, (0, *scalar))]
        }
        ModernTypeDecl::Enum(decl) => {
            vec![raw_scalar::setter(scalar_context, name, (0, decl.storage))]
        }
        ModernTypeDecl::Bits(decl) => {
            vec![raw_scalar::setter(scalar_context, name, (0, decl.storage))]
        }
        ModernTypeDecl::BitsReference(path) => {
            vec![bits::setter(bits::ViewContext {
                marker,
                name,
                offset,
                path,
                facade,
                endian,
            })]
        }
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
        ModernTypeDecl::Union(union) => {
            union_geometry(union, facade).map_or_else(Vec::new, |geometry| {
                let width = &geometry.storage.width;
                let ty = &geometry.storage.ty;
                (0..geometry.slots)
                    .map(|index| {
                        let method = if field.scoped_union {
                            format_ident!("{name}_word{index}")
                        } else {
                            format_ident!("word{index}")
                        };
                        let relative = quote!((#index).saturating_mul(#width));
                        raw_scalar::dynamic_setter(scalar_context, &method, &relative, ty, width)
                    })
                    .collect()
            })
        }
    }
}
