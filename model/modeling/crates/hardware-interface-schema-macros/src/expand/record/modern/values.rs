use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};

use super::{layout::RecordPlan, scalar, union};

mod field;

pub(super) fn expand(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let marker = &decl.name;
    let visibility = &decl.visibility;
    let values_name = format_ident!("{marker}Values");
    let endian = &decl.endian;
    let declarations = plan
        .fields
        .iter()
        .filter_map(|field| value_declaration(marker, field, visibility, facade));
    let zero_fields = plan
        .fields
        .iter()
        .filter_map(|field| zero_field(marker, field, facade));
    let decoded = plan
        .fields
        .iter()
        .map(|placed| field::decode(marker, placed, facade, endian))
        .collect::<syn::Result<Vec<_>>>()?;
    let initialized = plan
        .fields
        .iter()
        .filter_map(|field| initialized_field(field));
    let normalizers = plan
        .fields
        .iter()
        .filter_map(|field| {
            let ModernTypeDecl::Union(union_decl) = &field.decl.ty else {
                return None;
            };
            Some((field, union_decl))
        })
        .map(|(field, union_decl)| {
            let union_name = &field.decl.name;
            let root = quote!(normalized);
            let tag = union::tag_info(decl, &union_decl.tag, &root)?;
            let access = tag.access;
            Ok(quote!(#access = normalized.#union_name.tag();))
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let encoded = plan
        .fields
        .iter()
        .map(|placed| field::encode(marker, placed, facade, endian));
    let constant_assertions = plan.fields.iter().filter_map(|field| {
        let ModernTypeDecl::Scalar {
            scalar,
            constant: Some(path),
        } = &field.decl.ty
        else {
            return None;
        };
        let path = scalar::record_size_path(path);
        let maximum = usize::try_from(scalar.maximum()).map_or(usize::MAX, |maximum| maximum);
        Some(quote!(
            const _: () = assert!(#path <= #maximum, "record constant exceeds scalar storage");
        ))
    });
    Ok(quote! {
        #(#constant_assertions)*
        #[doc = "Generated owned semantic record value."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #values_name { #(#declarations,)* }
        impl #values_name {
            #[doc = "All-zero semantic value with first enum and union variants."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            #[doc = "Decodes one complete semantic record."]
            pub fn from_bytes(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..#marker::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #marker::RECORD_SIZE, actual })?;
                #(#decoded)*
                ::core::result::Result::Ok(Self { #(#initialized,)* })
            }
            #[doc = "Encodes into one exact inferred-size byte array."]
            pub fn to_bytes(self) -> ::core::result::Result<[u8; #marker::RECORD_SIZE], #facade::SchemaCodecError> {
                self.encoded()
            }
            #[doc = "Encodes atomically into caller-owned storage."]
            pub fn encode_into(&self, bytes: &mut [u8]) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let destination = bytes.get_mut(..#marker::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #marker::RECORD_SIZE, actual })?;
                let encoded = self.encoded()?;
                destination.copy_from_slice(&encoded);
                ::core::result::Result::Ok(())
            }
            fn encoded(&self) -> ::core::result::Result<[u8; #marker::RECORD_SIZE], #facade::SchemaCodecError> {
                let mut normalized = *self;
                #(#normalizers)*
                let mut encoded = [0_u8; #marker::RECORD_SIZE];
                #(#encoded)*
                ::core::result::Result::Ok(encoded)
            }
        }
        impl ::core::default::Default for #values_name {
            fn default() -> Self { Self::ZERO }
        }
    })
}

fn value_declaration(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> Option<TokenStream> {
    let name = &field.decl.name;
    let ty = match &field.decl.ty {
        ModernTypeDecl::Scalar {
            constant: Some(_), ..
        } => return None,
        ModernTypeDecl::Scalar { scalar, .. } => scalar::ty(*scalar),
        ModernTypeDecl::Enum(_) => {
            let ty = scalar::type_name(marker, name, "");
            quote!(#ty)
        }
        ModernTypeDecl::Bits(_) | ModernTypeDecl::Union(_) => {
            let ty = scalar::type_name(marker, name, "Values");
            quote!(#ty)
        }
        ModernTypeDecl::BitsReference(path) => quote!(<#path as #facade::BitsLayout>::Value),
        ModernTypeDecl::Record(path) => quote!(<#path as #facade::RecordLayout>::Value),
    };
    Some(quote!(#[doc = "Generated semantic record field."] #visibility #name: #ty))
}

fn zero_field(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
) -> Option<TokenStream> {
    let name = &field.decl.name;
    let value = match &field.decl.ty {
        ModernTypeDecl::Scalar {
            constant: Some(_), ..
        } => return None,
        ModernTypeDecl::Scalar { .. } => quote!(0),
        ModernTypeDecl::Enum(_) => {
            let ty = scalar::type_name(marker, name, "");
            quote!(#ty::ZERO)
        }
        ModernTypeDecl::Bits(_) | ModernTypeDecl::Union(_) => {
            let ty = scalar::type_name(marker, name, "Values");
            quote!(#ty::ZERO)
        }
        ModernTypeDecl::BitsReference(path) => quote!(<#path as #facade::BitsLayout>::ZERO),
        ModernTypeDecl::Record(path) => {
            quote!(<#path as #facade::RecordLayout>::ZERO)
        }
    };
    Some(quote!(#name: #value))
}

fn initialized_field(field: &super::layout::PlacedField<'_>) -> Option<TokenStream> {
    if matches!(
        field.decl.ty,
        ModernTypeDecl::Scalar {
            constant: Some(_),
            ..
        }
    ) {
        None
    } else {
        let name = &field.decl.name;
        Some(quote!(#name))
    }
}
