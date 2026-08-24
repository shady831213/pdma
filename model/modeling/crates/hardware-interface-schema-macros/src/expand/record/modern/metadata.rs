use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};

use super::{layout::RecordPlan, layout::union_geometry, scalar};

pub(super) fn expand(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let alignment = &plan.alignment;
    let extent = &plan.extent;
    let offsets = plan.fields.iter().map(|field| {
        let name = &field.offset_name;
        let value = &field.offset;
        quote!(#[doc = "Inferred field byte offset."] pub const #name: usize = #value;)
    });
    let field_count_parts = plan.fields.iter().map(|field| match &field.decl.ty {
        ModernTypeDecl::Record(path) => quote!(#path::FIELD_COUNT),
        ModernTypeDecl::Union(union) => {
            let slots = union_geometry(union).map_or(0, |geometry| geometry.slots);
            quote!(#slots)
        }
        ModernTypeDecl::Scalar { .. }
        | ModernTypeDecl::Enum(_)
        | ModernTypeDecl::Bits(_)
        | ModernTypeDecl::BitsReference(_) => {
            quote!(1_usize)
        }
    });
    let fillers = plan
        .fields
        .iter()
        .map(|field| fill_field(marker, field, facade, &decl.endian));
    let word_offsets = plan.fields.iter().flat_map(|field| {
        let Some(geometry) = union_geometry_for(field) else {
            return Vec::new();
        };
        let base = &field.offset_name;
        (0..geometry.slots)
            .map(|index| {
                let name = format_ident!("WORD{index}_OFFSET");
                let relative = index * geometry.storage.width();
                quote!(#[doc = "Generated raw union-storage slot offset."] pub const #name: usize = Self::#base.saturating_add(#relative);)
            })
            .collect::<Vec<_>>()
    });
    quote! {
        impl #marker {
            #(#offsets)*
            #(#word_offsets)*
            #[doc = "Number of flattened scalar storage fields."]
            pub const FIELD_COUNT: usize = 0_usize #(+ #field_count_parts)*;
            #[doc = "Inferred record alignment."]
            pub const RECORD_ALIGNMENT: usize = #alignment;
            #[doc = "Inferred aligned record byte size."]
            pub const RECORD_SIZE: usize = #extent;
            #[doc = "Flattened scalar storage metadata."]
            pub const FIELDS: [#facade::FieldSpec; Self::FIELD_COUNT] = {
                let mut fields = [#facade::FieldSpec::new(0, "", 0, #facade::ScalarKind::U8); Self::FIELD_COUNT];
                let mut output_index = 0_usize;
                #(#fillers)*
                fields
            };
            #[doc = "Complete inferred record specification."]
            pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(
                #marker_name,
                Self::RECORD_SIZE,
                Self::RECORD_ALIGNMENT,
                &Self::FIELDS,
                &[],
            );
        }
        const _: () = assert!(#marker::FIELD_COUNT <= 65_535, "record has too many generated fields");
        const _: () = assert!(#marker::RECORD_SIZE != usize::MAX, "record inferred layout overflows usize");
    }
}

fn fill_field(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let name = field.decl.name.to_string();
    let offset = &field.offset_name;
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => {
            fill_scalar(marker, &name, offset, *scalar, facade, endian)
        }
        ModernTypeDecl::Enum(decl) => {
            fill_scalar(marker, &name, offset, decl.storage, facade, endian)
        }
        ModernTypeDecl::Bits(decl) => {
            fill_scalar(marker, &name, offset, decl.storage, facade, endian)
        }
        ModernTypeDecl::BitsReference(path) => quote!({
            let id = match #facade::inferred_field_id(output_index) { Some(id) => id, None => 0 };
            fields[output_index] = #facade::FieldSpec::new(
                id,
                #name,
                #marker::#offset,
                <#path as #facade::BitsLayout>::SCALAR_KIND,
            )
                .with_endian(#facade::Endian::#endian)
                .with_declaration_index(output_index);
            output_index += 1;
        }),
        ModernTypeDecl::Record(path) => quote!({
            let child = #path::SPEC.fields();
            let mut child_index = 0_usize;
            while child_index < #path::FIELD_COUNT {
                let id = match #facade::inferred_field_id(output_index) { Some(id) => id, None => 0 };
                fields[output_index] = child[child_index]
                    .with_id(id)
                    .with_offset(#marker::#offset.saturating_add(child[child_index].offset()))
                    .with_declaration_index(output_index);
                child_index += 1;
                output_index += 1;
            }
        }),
        ModernTypeDecl::Union(union) => {
            let Some(geometry) = union_geometry(union) else {
                return TokenStream::new();
            };
            let kind = scalar::kind(geometry.storage);
            let width = geometry.storage.width();
            let slots = (0..geometry.slots).map(|index| {
                let slot_name = format!("word{index}");
                let relative = index * width;
                quote!({
                    let id = match #facade::inferred_field_id(output_index) { Some(id) => id, None => 0 };
                    fields[output_index] = #facade::FieldSpec::new(id, #slot_name, #marker::#offset.saturating_add(#relative), #facade::ScalarKind::#kind)
                        .with_endian(#facade::Endian::#endian)
                        .with_declaration_index(output_index);
                    output_index += 1;
                })
            });
            quote!(#(#slots)*)
        }
    }
}

fn fill_scalar(
    marker: &syn::Ident,
    name: &str,
    offset: &syn::Ident,
    scalar_type: crate::internal::syntax::ScalarType,
    facade: &TokenStream,
    endian: &syn::Ident,
) -> TokenStream {
    let kind = scalar::kind(scalar_type);
    quote!({
        let id = match #facade::inferred_field_id(output_index) { Some(id) => id, None => 0 };
        fields[output_index] = #facade::FieldSpec::new(id, #name, #marker::#offset, #facade::ScalarKind::#kind)
            .with_endian(#facade::Endian::#endian)
            .with_declaration_index(output_index);
        output_index += 1;
    })
}

fn union_geometry_for(
    field: &super::layout::PlacedField<'_>,
) -> Option<super::layout::UnionGeometry> {
    let ModernTypeDecl::Union(union) = &field.decl.ty else {
        return None;
    };
    union_geometry(union)
}
