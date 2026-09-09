use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::OrderedMapDecl;

use super::{ExpansionContext, Geometry, access_tree::Expansion as AccessTreeExpansion};

pub(super) struct GetterPair {
    pub(super) immutable: TokenStream,
    pub(super) mutable: TokenStream,
}

pub(super) fn leaf_getters(
    method: &syn::Ident,
    item: &Geometry,
    context: &ExpansionContext<'_>,
) -> GetterPair {
    let facade = context.facade;
    let marker = context.marker;
    let mutable_method = format_ident!("{}_mut", method);
    let definition = &item.definition;
    let placement = &item.placement;
    let rust_docs = &item.rust_docs;
    let count = item.count;
    let (parameters, check, instance) = if count == 1 {
        (quote!(), quote!(), quote!(0usize))
    } else {
        (
            quote!(instance: usize),
            quote!(if instance >= #count { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: instance }); }),
            quote!(instance),
        )
    };
    GetterPair {
        immutable: quote! {
            #rust_docs
            #[doc = "Returns one checked immutable child-layout view."]
            pub fn #method(&self, #parameters) -> ::core::result::Result<<#definition as #facade::RegisterLayout>::View<'_>, #facade::SchemaCodecError> {
                #check
                let range = #marker::__placement_range(#marker::#placement, #instance)?;
                let start = range.start;
                let end = range.end;
                let child = self.bytes.get(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: self.bytes.len() })?;
                <#definition as #facade::RegisterLayout>::view_at(child, #facade::SchemaCoordinate::Byte(start))
            }
        },
        mutable: quote! {
            #rust_docs
            #[doc = "Returns one checked mutable child-layout view."]
            pub fn #mutable_method(&mut self, #parameters) -> ::core::result::Result<<#definition as #facade::RegisterLayout>::ViewMut<'_>, #facade::SchemaCodecError> {
                #check
                let range = #marker::__placement_range(#marker::#placement, #instance)?;
                let start = range.start;
                let end = range.end;
                let actual = self.bytes.len();
                let child = self.bytes.get_mut(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                <#definition as #facade::RegisterLayout>::view_mut_at(child, #facade::SchemaCoordinate::Byte(start))
            }
        },
    }
}

pub(super) fn expand(
    decl: &OrderedMapDecl,
    context: &ExpansionContext<'_>,
    geometry: &[Geometry],
    access_tree: AccessTreeExpansion,
) -> TokenStream {
    let facade = context.facade;
    let view_name = context.view_name;
    let view_mut_name = context.view_mut_name;
    let extent = context.extent;
    let visibility = &decl.visibility;
    let tree_items = access_tree.items;
    let root_immutable = access_tree.root_immutable;
    let root_mutable = access_tree.root_mutable;
    let mut getters = Vec::new();
    let mut mutable_getters = Vec::new();
    for item in geometry {
        let pair = leaf_getters(&item.symbol, item, context);
        getters.push(pair.immutable);
        mutable_getters.push(pair.mutable);
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
            #root_immutable
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
            #root_immutable
            #root_mutable
        }
        #tree_items
    }
}
