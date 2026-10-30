use super::scalar;
use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl, TaggedUnionTagPath};
use proc_macro2::TokenStream;
use quote::quote;

pub(in super::super) struct TagInfo {
    pub(in super::super) ty: TokenStream,
    pub(in super::super) access: TokenStream,
}

pub(in super::super) fn tag_info(
    record: &ModernRecordDecl,
    path: &TaggedUnionTagPath,
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
    let ty = match &field.ty {
        ModernTypeDecl::Codec(definition) | ModernTypeDecl::BitsReference(definition) => {
            let ty = scalar::suffixed_path(definition, second);
            quote!(#ty)
        }
        ModernTypeDecl::Bits(_) => {
            let owner = scalar::type_name(&record.name, first, "Values");
            let ty = scalar::type_name(&owner, second, "");
            quote!(#ty)
        }
        ModernTypeDecl::Scalar { .. }
        | ModernTypeDecl::Enum(_)
        | ModernTypeDecl::TaggedUnion(_) => {
            return Err(syn::Error::new(
                first.span(),
                "tag root must be a record or bits field",
            ));
        }
    };
    Ok(TagInfo {
        ty,
        access: quote!(#root.#first.#second),
    })
}
