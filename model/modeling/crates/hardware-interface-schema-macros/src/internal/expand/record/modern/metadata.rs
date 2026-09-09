use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};

use super::{layout::RecordPlan, layout::union_geometry};

mod field;
use field::fill_field;

pub(super) fn expand(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let documentation = decl.documentation.metadata(facade);
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
        let Some(geometry) = union_geometry_for(field, facade) else {
            return Vec::new();
        };
        let base = &field.offset_name;
        let width = &geometry.storage.width;
        (0..geometry.slots)
            .map(|index| {
                let name = format_ident!("WORD{index}_OFFSET");
                let relative = quote!((#index).saturating_mul(#width));
                quote!(#[doc = "Generated raw union-storage slot offset."] pub const #name: usize = Self::#base.saturating_add(#relative);)
            })
            .collect::<Vec<_>>()
    });
    quote! {
        impl #marker {
            #[doc = "Declared schema documentation."]
            pub const DOCUMENTATION: #facade::Documentation = #documentation;
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
            ).with_documentation(Self::DOCUMENTATION);
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
        let documentation = field.decl.documentation.metadata(facade);
        if let ModernTypeDecl::Record(path) = &field.decl.ty {
            let id_index = next_id;
            next_id += 1;
            placements.push(quote!({
                let id = match #facade::inferred_field_id(#id_index) { Some(id) => id, None => 0 };
                #facade::RecordPlacement::new(id, #name, #marker::#offset, &#path::SPEC)
                    .with_declaration_index(#declaration_index)
                    .with_documentation((#documentation).with_fallback(<#path as #facade::RecordLayout>::DOCUMENTATION))
            }));
            continue;
        }
        let filler = fill_field(
            marker,
            field,
            facade,
            &decl.endian,
            declaration_index,
            next_id,
        );
        let documentation = match &field.decl.ty {
            ModernTypeDecl::BitsReference(path) => {
                quote!((#documentation).with_fallback(<#path as #facade::BitsLayout>::DOCUMENTATION))
            }
            _ => documentation,
        };
        let bits = match &field.decl.ty {
            ModernTypeDecl::BitsReference(path) => quote!(<#path as #facade::BitsLayout>::FIELDS),
            ModernTypeDecl::Bits(bits) => {
                let specs = bits.fields.iter().enumerate().map(|(index, bit)| {
                    let name = bit.name.to_string();
                    let lsb = bit.start.0;
                    let width = crate::internal::expand::bits::width(bit);
                    let documentation = bit.documentation.metadata(facade);
                    quote!(#facade::BitsFieldSpec::place(
                        match #facade::inferred_field_id(#index) { Some(id) => id, None => 0 },
                        #name, #lsb, #facade::BitsFieldDefinition::new(#width),
                    ).with_declaration_index(#index).with_documentation(#documentation))
                });
                quote!({
                    const BITS: &[#facade::BitsFieldSpec] = &[#(#specs),*];
                    BITS
                })
            }
            _ => quote!(&[]),
        };
        fillers.push(quote!({
            let first_output = output_index;
            #filler
            let mut documented = first_output;
            while documented < output_index {
                fields[documented] = fields[documented].with_documentation(#documentation).with_bits(#bits);
                documented += 1;
            }
        }));
        next_id += match &field.decl.ty {
            ModernTypeDecl::Union(union) => {
                union_geometry(union, facade).map_or(0, |geometry| geometry.slots)
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

fn union_geometry_for(
    field: &super::layout::PlacedField<'_>,
    facade: &TokenStream,
) -> Option<super::layout::UnionGeometry> {
    let ModernTypeDecl::Union(union) = &field.decl.ty else {
        return None;
    };
    union_geometry(union, facade)
}
