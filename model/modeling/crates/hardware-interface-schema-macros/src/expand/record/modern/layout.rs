use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{
    ModernFieldDecl, ModernRecordDecl, ModernTypeDecl, ScalarType, UnionDecl,
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
}

pub(super) struct UnionGeometry {
    pub(super) storage: ScalarType,
    pub(super) slots: usize,
    pub(super) extent: usize,
}

impl<'a> RecordPlan<'a> {
    pub(super) fn new(decl: &'a ModernRecordDecl, facade: &TokenStream) -> Self {
        let mut fields = Vec::with_capacity(decl.fields.len());
        let mut cursor = quote!(0_usize);
        let mut alignments = Vec::with_capacity(decl.fields.len());
        for field in &decl.fields {
            let alignment = type_alignment(&field.ty);
            let extent = type_extent(&field.ty);
            let offset = quote!(#facade::inferred_align_up(#cursor, #alignment));
            let offset_name =
                format_ident!("{}_OFFSET", field.name.to_string().to_ascii_uppercase());
            cursor = quote!((#offset).saturating_add(#extent));
            alignments.push(alignment.clone());
            fields.push(PlacedField {
                decl: field,
                offset_name,
                offset,
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

pub(super) fn union_geometry(union: &UnionDecl) -> Option<UnionGeometry> {
    let storage = union
        .cases
        .iter()
        .flat_map(|case| case.fields.iter())
        .next()
        .map(|field| field.storage)?;
    let slots = union
        .cases
        .iter()
        .map(|case| case.fields.len())
        .max()
        .map_or(0, |slots| slots);
    Some(UnionGeometry {
        storage,
        slots,
        extent: slots * storage.width(),
    })
}

fn type_alignment(ty: &ModernTypeDecl) -> TokenStream {
    match ty {
        ModernTypeDecl::Scalar { scalar, .. } => width(*scalar),
        ModernTypeDecl::Enum(decl) => width(decl.storage),
        ModernTypeDecl::Bits(decl) => width(decl.storage),
        ModernTypeDecl::Record(path) => quote!(#path::RECORD_ALIGNMENT),
        ModernTypeDecl::Union(union) => {
            let width = union_geometry(union).map_or(1, |geometry| geometry.storage.width());
            quote!(#width)
        }
    }
}

fn type_extent(ty: &ModernTypeDecl) -> TokenStream {
    match ty {
        ModernTypeDecl::Scalar { scalar, .. } => width(*scalar),
        ModernTypeDecl::Enum(decl) => width(decl.storage),
        ModernTypeDecl::Bits(decl) => width(decl.storage),
        ModernTypeDecl::Record(path) => quote!(#path::RECORD_SIZE),
        ModernTypeDecl::Union(union) => {
            let extent = union_geometry(union).map_or(0, |geometry| geometry.extent);
            quote!(#extent)
        }
    }
}

fn width(scalar: ScalarType) -> TokenStream {
    let width = scalar.width();
    quote!(#width)
}
