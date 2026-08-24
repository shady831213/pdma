use proc_macro2::TokenStream;
use quote::{format_ident, quote};

#[derive(Clone, Copy)]
pub(super) struct ViewContext<'a> {
    pub(super) marker: &'a syn::Ident,
    pub(super) name: &'a syn::Ident,
    pub(super) offset: &'a syn::Ident,
    pub(super) path: &'a syn::Path,
    pub(super) facade: &'a TokenStream,
    pub(super) endian: &'a syn::Ident,
}

pub(super) fn getter(context: ViewContext<'_>) -> TokenStream {
    let ViewContext {
        marker,
        name,
        offset,
        path,
        facade,
        endian,
    } = context;
    quote! {
        #[doc = "Decodes one reusable typed bits field."]
        pub fn #name(&self) -> ::core::result::Result<<#path as #facade::BitsLayout>::Value, #facade::SchemaCodecError> {
            let start = #marker::#offset;
            let end = start.saturating_add(#path::EXTENT);
            let bytes = self.bytes.get(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
            <#path as #facade::BitsLayout>::decode_record(bytes, #facade::Endian::#endian, start)
        }
    }
}

pub(super) fn setter(context: ViewContext<'_>) -> TokenStream {
    let ViewContext {
        marker,
        name,
        offset,
        path,
        facade,
        endian,
    } = context;
    let setter = format_ident!("set_{name}");
    quote! {
        #[doc = "Encodes one reusable typed bits field."]
        pub fn #setter(&mut self, value: <#path as #facade::BitsLayout>::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> {
            let start = #marker::#offset;
            let end = start.saturating_add(#path::EXTENT);
            let actual = self.bytes.len();
            let bytes = self.bytes.get_mut(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
            <#path as #facade::BitsLayout>::encode_record(bytes, #facade::Endian::#endian, &value, start)
        }
    }
}
