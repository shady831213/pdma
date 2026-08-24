use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{crate_path, syntax::ModernRecordDecl, validate};

mod bits;
mod enumeration;
mod layout;
mod metadata;
mod scalar;
mod union;
mod values;
mod views;

pub(super) fn expand(decl: &ModernRecordDecl) -> syn::Result<TokenStream> {
    validate::record::modern::check(decl)?;
    let facade = crate_path::facade()?;
    let marker = &decl.name;
    let visibility = &decl.visibility;
    let values_name = format_ident!("{marker}Values");
    let view_name = format_ident!("{marker}View");
    let view_mut_name = format_ident!("{marker}ViewMut");
    let plan = layout::RecordPlan::new(decl, &facade);
    let type_items = decl.fields.iter().map(|field| match &field.ty {
        crate::internal::syntax::ModernTypeDecl::Enum(enum_decl) => {
            enumeration::expand(marker, &field.name, enum_decl, visibility, &facade)
        }
        crate::internal::syntax::ModernTypeDecl::Bits(bits_decl) => Ok(bits::expand(
            marker,
            &field.name,
            bits_decl,
            visibility,
            &facade,
        )),
        crate::internal::syntax::ModernTypeDecl::Union(union_decl) => union::expand(
            decl,
            &field.name,
            union_decl,
            visibility,
            &facade,
            &decl.endian,
        ),
        crate::internal::syntax::ModernTypeDecl::Scalar { .. }
        | crate::internal::syntax::ModernTypeDecl::Record(_) => Ok(TokenStream::new()),
    });
    let type_items = type_items.collect::<syn::Result<Vec<_>>>()?;
    let metadata = metadata::expand(decl, &plan, &facade);
    let values = values::expand(decl, &plan, &facade)?;
    let views = views::expand(decl, &plan, &facade);
    let marker_name = marker.to_string();
    Ok(quote! {
        #(#type_items)*
        #[doc = "Marker and metadata namespace for the generated inferred record."]
        #[derive(Clone, Copy, Debug)]
        #visibility struct #marker;
        #metadata
        #values
        #views
        impl #facade::RecordLayout for #marker {
            type Value = #values_name;
            type View<'a> = #view_name<'a>;
            type ViewMut<'a> = #view_mut_name<'a>;
            const NAME: &'static str = #marker_name;
            const ALIGNMENT: usize = Self::RECORD_ALIGNMENT;
            const EXTENT: usize = Self::RECORD_SIZE;
            const ZERO: Self::Value = #values_name::ZERO;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> {
                #values_name::from_bytes(bytes)
            }
            fn encode(bytes: &mut [u8], value: &Self::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                value.encode_into(bytes)
            }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> {
                #view_name::new(bytes)
            }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> {
                #view_mut_name::new(bytes)
            }
        }
    })
}
