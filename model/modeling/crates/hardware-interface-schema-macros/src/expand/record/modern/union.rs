use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{
    ModernRecordDecl, ModernTypeDecl, ScopedPath, UnionCaseDecl, UnionDecl,
};

use super::{layout::union_geometry, scalar};

mod arms;
mod case_field;
mod codec;
mod identity;

pub(super) struct TagInfo {
    pub(super) ty: TokenStream,
    pub(super) access: TokenStream,
}

pub(super) fn tag_info(
    record: &ModernRecordDecl,
    path: &ScopedPath,
    root: &TokenStream,
) -> syn::Result<TagInfo> {
    let Some(first) = path.segments.first() else {
        return Err(syn::Error::new(path.span, "tag path is empty"));
    };
    if path.segments.len() == 1 {
        let field = record
            .fields
            .iter()
            .find(|field| field.name == *first)
            .ok_or_else(|| syn::Error::new(first.span(), "tag field does not exist"))?;
        if !matches!(field.ty, ModernTypeDecl::Enum(_)) {
            return Err(syn::Error::new(first.span(), "tag field must be an enum"));
        }
        let ty = scalar::type_name(&record.name, first, "");
        return Ok(TagInfo {
            ty: quote!(#ty),
            access: quote!(#root.#first),
        });
    }
    let Some(second) = path.segments.get(1) else {
        return Err(syn::Error::new(path.span, "tag path is incomplete"));
    };
    let field = record
        .fields
        .iter()
        .find(|field| field.name == *first)
        .ok_or_else(|| syn::Error::new(first.span(), "tag record does not exist"))?;
    let ModernTypeDecl::Record(definition) = &field.ty else {
        return Err(syn::Error::new(first.span(), "tag root must be a record"));
    };
    let ty = scalar::suffixed_path(definition, second);
    Ok(TagInfo {
        ty: quote!(#ty),
        access: quote!(#root.#first.#second),
    })
}

pub(super) fn expand(
    record: &ModernRecordDecl,
    field: &syn::Ident,
    decl: &UnionDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> syn::Result<TokenStream> {
    let name = scalar::type_name(&record.name, field, "Values");
    let raw_name = scalar::type_name(&record.name, field, "Raw");
    let Some(geometry) = union_geometry(decl, facade) else {
        return Err(syn::Error::new(field.span(), "union has no storage"));
    };
    let storage = &geometry.storage;
    let quantum = &storage.width;
    let slots = geometry.slots;
    let extent = &geometry.extent;
    let root = quote!(value);
    let storage_assertions = decl.cases.iter().flat_map(|case| {
        case.fields.iter().map(|field| {
            let width = match &field.ty {
                crate::internal::syntax::UnionFieldType::Scalar(scalar_type) => {
                    let width = scalar_type.width();
                    quote!(#width)
                }
                crate::internal::syntax::UnionFieldType::Bits(path) => {
                    quote!(<#path as #facade::BitsLayout>::EXTENT)
                }
            };
            quote!(
                const _: () =
                    assert!(#width == #quantum, "union fields must use one container width");
            )
        })
    });
    let tag = tag_info(record, &decl.tag, &root)?;
    let tag_type = &tag.ty;
    let case_declarations = decl
        .cases
        .iter()
        .map(|case| case_declaration(record, field, case, visibility, facade));
    let case_identities = decl.cases.iter().map(|case| {
        let case_type = case_type(record, field, &case.name);
        identity::expand(&case_type, &case.fields, facade)
    });
    let declarations = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        let case_type = case_type(record, field, case_name);
        quote!(#[doc = "Generated tagged-union case."] #case_name(#case_type))
    });
    let arm_context = arms::Context {
        record,
        field,
        decl,
        tag_type,
        storage,
        slots,
        quantum,
        facade,
        endian,
    };
    let decode_arms = arms::decode(&arm_context);
    let encode_arms = arms::encode(&arm_context);
    let tag_arms = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        quote!(Self::#case_name(_) => #tag_type::#case_name)
    });
    let Some(first) = decl.cases.first() else {
        return Err(syn::Error::new(field.span(), "union requires a case"));
    };
    let first_name = &first.name;
    let first_type = case_type(record, field, first_name);
    let codec = codec::expand(codec::Context {
        visibility,
        values: &name,
        raw: &raw_name,
        tag: tag_type,
        first_case: first_name,
        extent,
        facade,
    });
    Ok(quote! {
        #(#storage_assertions)*
        #(#case_declarations)*
        #(#case_identities)*
        #[doc = "Generated semantic tagged-union value."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#declarations),* }
        impl #name {
            #[doc = "First declared case used by generated zero records."]
            pub const ZERO: Self = Self::#first_name(#first_type::ZERO);
            fn decode(bytes: &[u8], base: usize, tag: #tag_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                match tag { #(#decode_arms),* }
            }
            fn encode_into(&self, bytes: &mut [u8], base: usize) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                match self { #(#encode_arms),* }
                ::core::result::Result::Ok(())
            }
            #[doc = "Returns the tag selected by this semantic union case."]
            pub const fn tag(&self) -> #tag_type {
                match self { #(#tag_arms),* }
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        #codec
    })
}

fn case_declaration(
    record: &ModernRecordDecl,
    field: &syn::Ident,
    case: &UnionCaseDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> TokenStream {
    let case_type = case_type(record, field, &case.name);
    let fields = case.fields.iter().map(|field| {
        let name = &field.name;
        let ty = match &field.ty {
            crate::internal::syntax::UnionFieldType::Scalar(scalar_type) => {
                scalar::ty(*scalar_type)
            }
            crate::internal::syntax::UnionFieldType::Bits(path) => {
                quote!(<#path as #facade::BitsLayout>::Value)
            }
        };
        quote!(#[doc = "Generated semantic union-case field."] pub #name: #ty)
    });
    let constructor_parameters = case.fields.iter().map(|field| {
        let name = &field.name;
        let ty = match &field.ty {
            crate::internal::syntax::UnionFieldType::Scalar(scalar_type) => {
                scalar::ty(*scalar_type)
            }
            crate::internal::syntax::UnionFieldType::Bits(path) => {
                quote!(<#path as #facade::BitsLayout>::Value)
            }
        };
        quote!(#name: #ty)
    });
    let constructor_fields = case.fields.iter().map(|field| &field.name);
    let accessors = case.fields.iter().map(|field| {
        let name = &field.name;
        let ty = match &field.ty {
            crate::internal::syntax::UnionFieldType::Scalar(scalar_type) => {
                scalar::ty(*scalar_type)
            }
            crate::internal::syntax::UnionFieldType::Bits(path) => {
                quote!(<#path as #facade::BitsLayout>::Value)
            }
        };
        quote! {
            #[doc = "Returns one generated union-case field."]
            pub const fn #name(self) -> #ty { self.#name }
        }
    });
    let zero_fields = case.fields.iter().map(|field| {
        let name = &field.name;
        let value = match &field.ty {
            crate::internal::syntax::UnionFieldType::Scalar(_) => quote!(0),
            crate::internal::syntax::UnionFieldType::Bits(path) => {
                quote!(<#path as #facade::BitsLayout>::ZERO)
            }
        };
        quote!(#name: #value)
    });
    quote! {
        #[doc = "Generated semantic tagged-union case value."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #case_type { #(#fields,)* }
        impl #case_type {
            #[doc = "Creates one semantic union-case value."]
            pub const fn new(#(#constructor_parameters),*) -> Self { Self { #(#constructor_fields,)* } }
            #[doc = "All-zero semantic case value."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            #(#accessors)*
        }
        impl ::core::default::Default for #case_type {
            fn default() -> Self { Self::ZERO }
        }
    }
}

pub(super) fn case_type(
    record: &ModernRecordDecl,
    field: &syn::Ident,
    case: &syn::Ident,
) -> syn::Ident {
    let union = scalar::type_name(&record.name, field, "");
    format_ident!("{union}{case}Values")
}
