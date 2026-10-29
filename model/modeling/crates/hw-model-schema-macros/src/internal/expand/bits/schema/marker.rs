use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::{SchemaContext, values::narrow_raw};

pub(super) fn expand(context: &SchemaContext<'_>) -> syn::Result<TokenStream> {
    let facade = &context.facade;
    let documentation = context.decl.documentation.metadata(facade);
    let rust_docs = context.decl.documentation.rust_docs();
    let visibility = &context.decl.visibility;
    let marker = &context.decl.name;
    let marker_name = marker.to_string();
    let values = &context.values;
    let field_id = &context.field_id;
    let bit_width = context.bit_width;
    let extent = context.extent;
    let declared_mask = context.declared_mask;
    let raw_type = format_ident!("u{}", extent * 8);
    let narrowed_raw = narrow_raw(&raw_type, extent);
    let scalar_kind = format_ident!("U{}", extent * 8);
    let specs = specs(context)?;
    let field_count = context.decl.fields.len();
    let member_identities = context.decl.fields.iter().map(|field| {
        let identity = format_ident!("{}_IDENTITY", field.name.to_string().to_ascii_uppercase());
        let member = field.name.to_string();
        quote! {
            #[doc = "Stable identity for one declared bit member."]
            pub const #identity: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(#marker_name, #member);
        }
    });
    Ok(quote! {
        #facade::__register_schema!(<#marker as #facade::BitsLayout>::schema_definition);
        impl #facade::graph::DescribeSchema for #marker {
            fn schema_definition() -> #facade::graph::Definition {
                <Self as #facade::BitsLayout>::schema_definition()
            }
        }
        #rust_docs
        #[doc = "Marker and metadata namespace for one reusable bits schema."]
        #[derive(Clone, Copy, Debug)]
        #visibility struct #marker;
        impl #marker {
            #[doc = "Declared schema documentation."]
            pub const DOCUMENTATION: #facade::Documentation = #documentation;
            #[doc = "Stable identity for reserved bits outside declared members."]
            pub const RESERVED_IDENTITY: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(#marker_name, "reserved");
            #(#member_identities)*
            #[doc = "Highest exclusive declared bit endpoint."]
            pub const BIT_WIDTH: usize = #bit_width;
            #[doc = "Smallest supported scalar byte extent containing every field."]
            pub const EXTENT: usize = #extent;
            #[doc = "Mask containing every declared bit."]
            pub const DECLARED_MASK: u64 = #declared_mask;
            #[doc = "Stable fields in declaration order."]
            pub const FIELDS: [#facade::BitsFieldSpec; #field_count] = [#(#specs),*];
        }
        impl #facade::BitsLayout for #marker {
            type Value = #values;
            type Storage = #raw_type;
            type FieldId = #field_id;
            const NAME: &'static str = #marker_name;
            const DOCUMENTATION: #facade::Documentation = Self::DOCUMENTATION;
            const BIT_WIDTH: usize = #bit_width;
            const EXTENT: usize = #extent;
            const DECLARED_MASK: u64 = #declared_mask;
            const SCALAR_KIND: #facade::ScalarKind = #facade::ScalarKind::#scalar_kind;
            const ZERO: Self::Value = #values::ZERO;
            const FIELDS: &'static [#facade::BitsFieldSpec] = &Self::FIELDS;
            fn decode_register(raw: u64) -> ::core::result::Result<Self::Value, #facade::RegisterValueError> { #values::from_raw(raw).map_err(#facade::RegisterValueError::from_schema_error) }
            fn encode_register(value: &Self::Value) -> ::core::result::Result<u64, #facade::RegisterValueError> {
                value.raw().map_err(#facade::BitsValueError::into_register_error)
            }
            fn decode_record(bytes: &[u8], endian: #facade::Endian, location: #facade::SchemaLocation) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> {
                let raw: #raw_type = #facade::read_scalar(bytes, endian)?;
                let raw = u64::from(raw);
                let reserved = raw & !#declared_mask;
                if reserved != 0 {
                    let mut first_reserved = 0_usize;
                    while reserved & (1_u64 << first_reserved) == 0 {
                        first_reserved += 1;
                    }
                    let ::core::option::Option::Some(location) = location.checked_rebase(#facade::SchemaCoordinate::Bit(first_reserved)) else {
                        return ::core::result::Result::Err(#facade::SchemaCodecError::CoordinateRebaseOverflow {
                            location,
                            base: #facade::SchemaCoordinate::Bit(first_reserved),
                        });
                    };
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                        location,
                        observed: reserved,
                    });
                }
                #values::from_raw(raw).map_err(|error| error.rebased(location.coordinate()))
            }
            fn encode_record(bytes: &mut [u8], endian: #facade::Endian, value: &Self::Value, location: #facade::SchemaLocation) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let raw = value
                    .raw()
                    .map_err(|error| error.into_schema_error().rebased(location.coordinate()))?;
                let raw: #raw_type = #narrowed_raw;
                #facade::write_scalar(bytes, endian, raw)
            }
        }
    })
}

fn specs(context: &SchemaContext<'_>) -> syn::Result<Vec<TokenStream>> {
    let facade = &context.facade;
    context
        .decl
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let id = u16::try_from(index.saturating_add(1)).map_err(|_| {
                syn::Error::new(field.name.span(), "bits schema field ID exceeds u16")
            })?;
            let name = field.name.to_string();
            let documentation = field.documentation.metadata(facade);
            let lsb = field.start.0;
            let width = super::super::width(field);
            let enumeration = field.enumeration.as_ref().map(|decl| {
                let metadata = crate::internal::expand::enumeration::metadata(decl, facade);
                quote!(.with_enum(#metadata))
            });
            Ok(quote!(#facade::BitsFieldSpec::place(
                #id, #name, #lsb, #facade::BitsFieldDefinition::new(#width)#enumeration,
            ).with_declaration_index(#index).with_documentation(#documentation)))
        })
        .collect()
}
