use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::OrderedMapDecl;

use super::{ExpansionContext, Geometry};

pub(super) fn expand(
    decl: &OrderedMapDecl,
    context: &ExpansionContext<'_>,
    geometry: &[Geometry],
) -> TokenStream {
    let facade = context.facade;
    let marker = context.marker;
    let view_name = context.view_name;
    let view_mut_name = context.view_mut_name;
    let extent = context.extent;
    let visibility = &decl.visibility;
    let mut getters = Vec::new();
    let mut mutable_getters = Vec::new();
    for item in geometry {
        let method = &item.symbol;
        let mutable_method = format_ident!("{}_mut", method);
        let definition = &item.definition;
        let offset = &item.offset;
        let stride = &item.stride;
        let count = item.count;
        let (parameters, check, start) = if count == 1 {
            (quote!(), quote!(), quote!(#marker::#offset))
        } else {
            (
                quote!(instance: usize),
                quote!(if instance >= #count { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: instance }); }),
                quote!(#marker::#offset + instance * #marker::#stride),
            )
        };
        getters.push(quote! {
            #[doc = "Returns one checked immutable child-layout view."]
            pub fn #method(&self, #parameters) -> ::core::result::Result<<#definition as #facade::RegisterLayout>::View<'_>, #facade::SchemaCodecError> {
                #check
                let start = #start;
                let end = start + <#definition as #facade::RegisterLayout>::EXTENT;
                let child = self.bytes.get(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
                <#definition as #facade::RegisterLayout>::view(child)
            }
        });
        mutable_getters.push(quote! {
            #[doc = "Returns one checked mutable child-layout view."]
            pub fn #mutable_method(&mut self, #parameters) -> ::core::result::Result<<#definition as #facade::RegisterLayout>::ViewMut<'_>, #facade::SchemaCodecError> {
                #check
                let start = #start;
                let end = start + <#definition as #facade::RegisterLayout>::EXTENT;
                let actual = self.bytes.len();
                let child = self.bytes.get_mut(start..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                <#definition as #facade::RegisterLayout>::view_mut(child)
            }
        });
    }
    quote! {
        #[doc = "Checked immutable byte view for the generated ordered map."]
        #[derive(Debug)]
        #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> {
            #[doc = "Creates a checked immutable map view."]
            pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                Ok(Self { bytes })
            }
            #(#getters)*
        }
        #[doc = "Checked mutable byte view for the generated ordered map."]
        #[derive(Debug)]
        #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> {
            #[doc = "Creates a checked mutable map view."]
            pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get_mut(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                Ok(Self { bytes })
            }
            #(#getters)*
            #(#mutable_getters)*
        }
    }
}
