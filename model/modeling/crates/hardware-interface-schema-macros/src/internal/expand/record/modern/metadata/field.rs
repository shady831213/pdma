use super::super::{layout::union_geometry, scalar};
use crate::internal::syntax::ModernTypeDecl;
use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn fill_field(
    marker: &syn::Ident,
    field: &super::super::layout::PlacedField<'_>,
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
            let id = match #facade::inferred_field_id(#id_index) { ::core::option::Option::Some(id) => id, ::core::option::Option::None => 0 };
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
            let Some(geometry) = union_geometry(union, facade) else {
                return TokenStream::new();
            };
            let kind = &geometry.storage.kind;
            let width = &geometry.storage.width;
            let slots = (0..geometry.slots).map(|index| {
                let slot_id = id_index + index;
                let slot_name = if field.scoped_union { format!("{name}_word{index}") } else { format!("word{index}") };
                let relative = quote!((#index).saturating_mul(#width));
                quote!({
                    let id = match #facade::inferred_field_id(#slot_id) { ::core::option::Option::Some(id) => id, ::core::option::Option::None => 0 };
                    fields[output_index] = #facade::FieldSpec::new(id, #slot_name, #marker::#offset.saturating_add(#relative), #kind)
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
        let id = match #facade::inferred_field_id(#id_index) { ::core::option::Option::Some(id) => id, ::core::option::Option::None => 0 };
        fields[output_index] = #facade::FieldSpec::new(id, #name, #marker::#offset, #facade::ScalarKind::#kind)
            .with_endian(#facade::Endian::#endian)
            .with_declaration_index(#declaration_index);
        output_index += 1;
    })
}
