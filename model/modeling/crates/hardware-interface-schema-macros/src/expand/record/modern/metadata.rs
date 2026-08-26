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
        let offset_name = &field.offset_name;
        let identity_name = format_ident!(
            "{}_IDENTITY",
            field.decl.name.to_string().to_ascii_uppercase()
        );
        let field_name = field.decl.name.to_string();
        let value = &field.offset;
        quote! {
            #[doc = "Inferred field byte offset."]
            pub const #offset_name: usize = #value;
            #[doc = "Stable generated identity for the declared field."]
            pub const #identity_name: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(stringify!(#marker), #field_name);
        }
    });
    let metadata = metadata_parts(decl, plan, facade);
    let field_count = metadata.field_count;
    let record_count = metadata.placements.len();
    let fillers = metadata.fillers;
    let placements = metadata.placements;
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
            #[doc = "Number of direct scalar storage fields."]
            pub const FIELD_COUNT: usize = #field_count;
            #[doc = "Number of directly nested record placements."]
            pub const RECORD_COUNT: usize = #record_count;
            #[doc = "Inferred record alignment."]
            pub const RECORD_ALIGNMENT: usize = #alignment;
            #[doc = "Inferred aligned record byte size."]
            pub const RECORD_SIZE: usize = #extent;
            #[doc = "Direct scalar storage metadata."]
            pub const FIELDS: [#facade::FieldSpec; Self::FIELD_COUNT] = {
                let mut fields = [#facade::FieldSpec::new(0, "", 0, #facade::ScalarKind::U8); Self::FIELD_COUNT];
                let mut output_index = 0_usize;
                #(#fillers)*
                fields
            };
            #[doc = "Direct nested record placements."]
            pub const RECORDS: [#facade::RecordPlacement; Self::RECORD_COUNT] = [#(#placements),*];
            #[doc = "Complete inferred record specification."]
            pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(
                #marker_name,
                Self::RECORD_SIZE,
                Self::RECORD_ALIGNMENT,
                &Self::FIELDS,
                &Self::RECORDS,
            );
        }
        const _: () = assert!(#marker::FIELD_COUNT <= 65_535, "record has too many generated fields");
        const _: () = assert!(#marker::RECORD_SIZE != usize::MAX, "record inferred layout overflows usize");
    }
}

struct MetadataParts {
    field_count: usize,
    fillers: Vec<TokenStream>,
    placements: Vec<TokenStream>,
}

fn metadata_parts(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> MetadataParts {
    let marker = &decl.name;
    let mut next_id = 0_usize;
    let mut fillers = Vec::new();
    let mut placements = Vec::new();
    for (declaration_index, field) in plan.fields.iter().enumerate() {
        let name = field.decl.name.to_string();
        let offset = &field.offset_name;
        if let ModernTypeDecl::Record(path) = &field.decl.ty {
            let id_index = next_id;
            next_id += 1;
            placements.push(quote!({
                let id = match #facade::inferred_field_id(#id_index) { Some(id) => id, None => 0 };
                #facade::RecordPlacement::new(id, #name, #marker::#offset, &#path::SPEC)
                    .with_declaration_index(#declaration_index)
            }));
            continue;
        }
        fillers.push(fill_field(
            marker,
            field,
            facade,
            &decl.endian,
            declaration_index,
            next_id,
        ));
        next_id += match &field.decl.ty {
            ModernTypeDecl::Union(union) => {
                union_geometry(union).map_or(0, |geometry| geometry.slots)
            }
            _ => 1,
        };
    }
    MetadataParts {
        field_count: next_id.saturating_sub(placements.len()),
        fillers,
        placements,
    }
}

fn fill_field(
    marker: &syn::Ident,
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
    endian: &syn::Ident,
    declaration_index: usize,
    id_index: usize,
) -> TokenStream {
    let name = field.decl.name.to_string();
    let offset = &field.offset_name;
    match &field.decl.ty {
        ModernTypeDecl::Scalar { scalar, .. } => fill_scalar(ScalarFill {
            marker,
            name: &name,
            offset,
            scalar_type: *scalar,
            facade,
            endian,
            declaration_index,
            id_index,
        }),
        ModernTypeDecl::Enum(decl) => fill_scalar(ScalarFill {
            marker,
            name: &name,
            offset,
            scalar_type: decl.storage,
            facade,
            endian,
            declaration_index,
            id_index,
        }),
        ModernTypeDecl::Bits(decl) => fill_scalar(ScalarFill {
            marker,
            name: &name,
            offset,
            scalar_type: decl.storage,
            facade,
            endian,
            declaration_index,
            id_index,
        }),
        ModernTypeDecl::BitsReference(path) => quote!({
            let id = match #facade::inferred_field_id(#id_index) { Some(id) => id, None => 0 };
            fields[output_index] = #facade::FieldSpec::new(
                id,
                #name,
                #marker::#offset,
                <#path as #facade::BitsLayout>::SCALAR_KIND,
            )
                .with_endian(#facade::Endian::#endian)
                .with_declaration_index(#declaration_index);
            output_index += 1;
        }),
        ModernTypeDecl::Record(_) => TokenStream::new(),
        ModernTypeDecl::Union(union) => {
            let Some(geometry) = union_geometry(union) else {
                return TokenStream::new();
            };
            let kind = scalar::kind(geometry.storage);
            let width = geometry.storage.width();
            let slots = (0..geometry.slots).map(|index| {
                let slot_id = id_index + index;
                let slot_name = format!("word{index}");
                let relative = index * width;
                quote!({
                    let id = match #facade::inferred_field_id(#slot_id) { Some(id) => id, None => 0 };
                    fields[output_index] = #facade::FieldSpec::new(id, #slot_name, #marker::#offset.saturating_add(#relative), #facade::ScalarKind::#kind)
                        .with_endian(#facade::Endian::#endian)
                        .with_declaration_index(#declaration_index);
                    output_index += 1;
                })
            });
            quote!(#(#slots)*)
        }
    }
}

#[derive(Clone, Copy)]
struct ScalarFill<'a> {
    marker: &'a syn::Ident,
    name: &'a str,
    offset: &'a syn::Ident,
    scalar_type: crate::internal::syntax::ScalarType,
    facade: &'a TokenStream,
    endian: &'a syn::Ident,
    declaration_index: usize,
    id_index: usize,
}

fn fill_scalar(input: ScalarFill<'_>) -> TokenStream {
    let ScalarFill {
        marker,
        name,
        offset,
        scalar_type,
        facade,
        endian,
        declaration_index,
        id_index,
    } = input;
    let kind = scalar::kind(scalar_type);
    quote!({
        let id = match #facade::inferred_field_id(#id_index) { Some(id) => id, None => 0 };
        fields[output_index] = #facade::FieldSpec::new(id, #name, #marker::#offset, #facade::ScalarKind::#kind)
            .with_endian(#facade::Endian::#endian)
            .with_declaration_index(#declaration_index);
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
