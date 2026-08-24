use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::ScalarType;

#[derive(Clone, Copy)]
pub(super) struct ViewContext<'a> {
    pub(super) marker: &'a syn::Ident,
    pub(super) offset: &'a syn::Ident,
    pub(super) facade: &'a TokenStream,
    pub(super) endian: &'a syn::Ident,
}

pub(super) fn getter(
    context: ViewContext<'_>,
    method: &syn::Ident,
    storage: (usize, ScalarType),
) -> TokenStream {
    let ViewContext {
        marker,
        offset,
        facade,
        endian,
    } = context;
    let (relative, scalar_type) = storage;
    let ty = super::super::scalar::ty(scalar_type);
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

pub(super) fn setter(
    context: ViewContext<'_>,
    method: &syn::Ident,
    storage: (usize, ScalarType),
) -> TokenStream {
    let ViewContext {
        marker,
        offset,
        facade,
        endian,
    } = context;
    let (relative, scalar_type) = storage;
    let setter = format_ident!("set_{method}");
    let ty = super::super::scalar::ty(scalar_type);
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
