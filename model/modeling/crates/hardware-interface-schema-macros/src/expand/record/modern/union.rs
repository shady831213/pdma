use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::syntax::{
    ModernRecordDecl, ModernTypeDecl, ScopedPath, UnionDecl, UnionFieldDecl,
};

use super::{layout::union_geometry, scalar};

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
    let Some(geometry) = union_geometry(decl) else {
        return Err(syn::Error::new(field.span(), "union has no storage"));
    };
    let storage = geometry.storage;
    let storage_type = scalar::ty(storage);
    let quantum = storage.width();
    let slots = geometry.slots;
    let root = quote!(value);
    let tag = tag_info(record, &decl.tag, &root)?;
    let tag_type = &tag.ty;
    let declarations = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        let fields = case.fields.iter().map(|field| {
            let name = &field.name;
            let ty = scalar::ty(field.scalar);
            quote!(#[doc = "Generated semantic union field."] #name: #ty)
        });
        quote!(#[doc = "Generated tagged-union case."] #case_name { #(#fields,)* })
    });
    let decode_arms = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        let reserved = (case.fields.len()..slots).map(|index| {
            let relative = index * quantum;
            let end = relative + quantum;
            quote!({
                let offset = base.saturating_add(#relative);
                let field = bytes.get(offset..base.saturating_add(#end)).ok_or(#facade::SchemaCodecError::BufferTooShort { required: base.saturating_add(#end), actual: bytes.len() })?;
                let value: #storage_type = #facade::read_scalar(field, #facade::Endian::#endian)?;
                let observed = u64::from(value);
                if observed != 0 {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved { offset, observed });
                }
            })
        });
        let fields = case.fields.iter().enumerate().map(|(index, field)| {
            decode_field(field, index * quantum, quantum, facade, endian)
        });
        quote!(#tag_type::#case_name => {
            #(#reserved)*
            ::core::result::Result::Ok(Self::#case_name { #(#fields,)* })
        })
    });
    let encode_arms = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        let names = case.fields.iter().map(|field| &field.name);
        let encoded =
            case.fields.iter().enumerate().map(|(index, field)| {
                encode_field(field, index * quantum, quantum, facade, endian)
            });
        quote!(Self::#case_name { #(#names,)* } => { #(#encoded)* })
    });
    let tag_arms = decl.cases.iter().map(|case| {
        let case_name = &case.name;
        quote!(Self::#case_name { .. } => #tag_type::#case_name)
    });
    let Some(first) = decl.cases.first() else {
        return Err(syn::Error::new(field.span(), "union requires a case"));
    };
    let first_name = &first.name;
    let first_zero = first.fields.iter().map(|field| {
        let name = &field.name;
        quote!(#name: 0)
    });
    Ok(quote! {
        #[doc = "Generated semantic tagged-union value."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#declarations),* }
        impl #name {
            #[doc = "First declared case used by generated zero records."]
            pub const ZERO: Self = Self::#first_name { #(#first_zero,)* };
            fn decode(bytes: &[u8], base: usize, tag: #tag_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                match tag { #(#decode_arms),* }
            }
            fn encode(&self, bytes: &mut [u8], base: usize) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                match self { #(#encode_arms),* }
                ::core::result::Result::Ok(())
            }
            fn tag(&self) -> #tag_type {
                match self { #(#tag_arms),* }
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
    })
}

fn decode_field(
    field: &UnionFieldDecl,
    relative: usize,
    quantum: usize,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let name = &field.name;
    let storage_type = scalar::ty(field.storage);
    let maximum = field.scalar.maximum();
    let narrowed = scalar::from_u64(field.scalar, quote!(observed));
    quote!(#name: {
        let offset = base.saturating_add(#relative);
        let end = offset.saturating_add(#quantum);
        let field = bytes.get(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
        let stored: #storage_type = #facade::read_scalar(field, #facade::Endian::#endian)?;
        let observed = u64::from(stored);
        if observed > #maximum {
            return ::core::result::Result::Err(#facade::SchemaCodecError::NarrowingOverflow { offset, observed, maximum: #maximum });
        }
        #narrowed
    })
}

fn encode_field(
    field: &UnionFieldDecl,
    relative: usize,
    quantum: usize,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let name = &field.name;
    let stored = scalar::from_u64(field.storage, quote!(u64::from(*#name)));
    quote!({
        let offset = base.saturating_add(#relative);
        let end = offset.saturating_add(#quantum);
        let actual = bytes.len();
        let field = bytes.get_mut(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
        #facade::write_scalar(field, #facade::Endian::#endian, #stored)?;
    })
}
