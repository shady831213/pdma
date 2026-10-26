use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{
    ModernFieldDecl, ModernRecordDecl, ModernTypeDecl, ScalarType, TaggedUnionDecl,
    TaggedUnionFieldType,
};

pub(super) struct RecordPlan<'a> {
    pub(super) fields: Vec<PlacedField<'a>>,
    pub(super) alignment: TokenStream,
    pub(super) extent: TokenStream,
}

pub(super) struct PlacedField<'a> {
    pub(super) decl: &'a ModernFieldDecl,
    pub(super) offset_name: syn::Ident,
    pub(super) offset: TokenStream,
    pub(super) scoped_tagged_union: bool,
}

pub(super) struct TaggedUnionGeometry {
    pub(super) storage: TaggedUnionStorage,
    pub(super) slots: usize,
    pub(super) extent: TokenStream,
}

pub(super) struct TaggedUnionStorage {
    pub(super) ty: TokenStream,
    pub(super) kind: TokenStream,
    pub(super) width: TokenStream,
}

impl<'a> RecordPlan<'a> {
    pub(super) fn new(decl: &'a ModernRecordDecl, facade: &TokenStream) -> Self {
        let mut fields = Vec::with_capacity(decl.fields.len());
        let scoped_tagged_union = decl
            .fields
            .iter()
            .filter(|field| matches!(field.ty, ModernTypeDecl::TaggedUnion(_)))
            .count()
            > 1;
        let mut cursor = quote!(0_usize);
        let mut alignments = Vec::with_capacity(decl.fields.len());
        for field in &decl.fields {
            let alignment = type_alignment(&field.ty, facade);
            let extent = type_extent(&field.ty, facade);
            let offset = quote!(#facade::inferred_align_up(#cursor, #alignment));
            let offset_name =
                format_ident!("{}_OFFSET", field.name.to_string().to_ascii_uppercase());
            cursor = quote!((#offset).saturating_add(#extent));
            alignments.push(alignment.clone());
            fields.push(PlacedField {
                decl: field,
                offset_name,
                offset,
                scoped_tagged_union,
            });
        }
        let alignment = decl.alignment.map_or_else(
            || {
                quote!({
                    let mut alignment = 1_usize;
                    #(if #alignments > alignment { alignment = #alignments; })*
                    alignment
                })
            },
            |(alignment, _)| quote!(#alignment),
        );
        let extent = quote!(#facade::inferred_align_up(#cursor, #alignment));
        Self {
            fields,
            alignment,
            extent,
        }
    }
}

pub(super) fn tagged_union_geometry(
    union: &TaggedUnionDecl,
    facade: &TokenStream,
) -> Option<TaggedUnionGeometry> {
    if union.cases.first()?.codec.is_some() {
        let codecs = union
            .cases
            .iter()
            .filter_map(|case| case.codec.as_ref())
            .collect::<Vec<_>>();
        let alignment = quote!({ let mut value = 1_usize; #(assert!(<#codecs as #facade::SchemaCodec>::ALIGNMENT.is_power_of_two(), "union case alignment must be a nonzero power of two");)* #(if <#codecs as #facade::SchemaCodec>::ALIGNMENT > value { value = <#codecs as #facade::SchemaCodec>::ALIGNMENT; })* value });
        let extent = quote!({ let mut value = 0_usize; #(if <#codecs as #facade::SchemaCodec>::EXTENT > value { value = <#codecs as #facade::SchemaCodec>::EXTENT; })* #facade::inferred_align_up(value, #alignment) });
        return Some(TaggedUnionGeometry {
            storage: TaggedUnionStorage {
                ty: quote!(u8),
                kind: quote!(#facade::ScalarKind::U8),
                width: alignment,
            },
            slots: 0,
            extent,
        });
    }
    let storage = union
        .cases
        .iter()
        .flat_map(|case| case.fields.iter())
        .next()
        .map(|field| tagged_union_storage(&field.ty, facade))?;
    let slots = union
        .cases
        .iter()
        .map(|case| case.fields.len())
        .max()
        .map_or(0, |slots| slots);
    let width = &storage.width;
    let extent = quote!((#width).saturating_mul(#slots));
    Some(TaggedUnionGeometry {
        storage,
        slots,
        extent,
    })
}

fn type_alignment(ty: &ModernTypeDecl, facade: &TokenStream) -> TokenStream {
    match ty {
        ModernTypeDecl::Scalar { scalar, .. } => width(*scalar),
        ModernTypeDecl::Enum(decl) => width(decl.storage),
        ModernTypeDecl::Bits(decl) => width(decl.storage),
        ModernTypeDecl::BitsReference(path) => quote!(#path::EXTENT),
        ModernTypeDecl::Codec(path) => quote!(<#path as #facade::SchemaCodec>::ALIGNMENT),
        ModernTypeDecl::TaggedUnion(union) => tagged_union_geometry(union, facade)
            .map_or_else(|| quote!(1_usize), |geometry| geometry.storage.width),
    }
}

fn type_extent(ty: &ModernTypeDecl, facade: &TokenStream) -> TokenStream {
    match ty {
        ModernTypeDecl::Scalar { scalar, .. } => width(*scalar),
        ModernTypeDecl::Enum(decl) => width(decl.storage),
        ModernTypeDecl::Bits(decl) => width(decl.storage),
        ModernTypeDecl::BitsReference(path) => quote!(#path::EXTENT),
        ModernTypeDecl::Codec(path) => quote!(<#path as #facade::SchemaCodec>::EXTENT),
        ModernTypeDecl::TaggedUnion(union) => tagged_union_geometry(union, facade)
            .map_or_else(|| quote!(0_usize), |geometry| geometry.extent),
    }
}

fn width(scalar: ScalarType) -> TokenStream {
    let width = scalar.width();
    quote!(#width)
}

fn tagged_union_storage(ty: &TaggedUnionFieldType, facade: &TokenStream) -> TaggedUnionStorage {
    match ty {
        TaggedUnionFieldType::Scalar(scalar_type) => {
            let ty = super::scalar::ty(*scalar_type);
            let kind = super::scalar::kind(*scalar_type);
            let width = scalar_type.width();
            TaggedUnionStorage {
                ty,
                kind: quote!(#facade::ScalarKind::#kind),
                width: quote!(#width),
            }
        }
        TaggedUnionFieldType::Bits(path) => TaggedUnionStorage {
            ty: quote!(<#path as #facade::BitsLayout>::Storage),
            kind: quote!(<#path as #facade::BitsLayout>::SCALAR_KIND),
            width: quote!(<#path as #facade::BitsLayout>::EXTENT),
        },
    }
}
