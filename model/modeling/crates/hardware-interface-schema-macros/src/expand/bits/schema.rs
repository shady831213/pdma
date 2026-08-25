use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{crate_path, syntax::BitsSchemaDecl, validate};

mod codec;

struct SchemaContext<'a> {
    decl: &'a BitsSchemaDecl,
    facade: TokenStream,
    values: syn::Ident,
    field_id: syn::Ident,
    bit_width: usize,
    extent: usize,
    declared_mask: u64,
}

pub(super) fn expand(decl: &BitsSchemaDecl) -> syn::Result<TokenStream> {
    validate::bits::check_schema(&decl.fields)?;
    let context = SchemaContext {
        decl,
        facade: crate_path::facade()?,
        values: format_ident!("{}Values", decl.name),
        field_id: format_ident!("{}FieldId", decl.name),
        bit_width: validate::bits::bit_width(&decl.fields),
        extent: validate::bits::extent(&decl.fields),
        declared_mask: validate::bits::declared_mask(&decl.fields),
    };
    let identities = identities(&context);
    let values = values(&context);
    let marker = marker(&context)?;
    Ok(quote!(#identities #values #marker))
}

fn identities(context: &SchemaContext<'_>) -> TokenStream {
    let visibility = &context.decl.visibility;
    let field_id = &context.field_id;
    let variants = context.decl.fields.iter().map(|field| {
        let variant = super::super::register::variant(&field.name.to_string());
        quote!(#[doc = "Stable generated field ID."] #variant)
    });
    quote! {
        #[doc = "Stable identities for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility enum #field_id { #(#variants),* }
    }
}

fn values(context: &SchemaContext<'_>) -> TokenStream {
    let facade = &context.facade;
    let visibility = &context.decl.visibility;
    let values = &context.values;
    let raw_name = format_ident!("{}Raw", context.decl.name);
    let raw_type = format_ident!("u{}", context.extent * 8);
    let declared_mask = context.declared_mask;
    let narrowed_raw = narrow_raw(&raw_type, context.extent);
    let declarations = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let ty = super::semantic_type(field);
        quote!(#[doc = "Generated typed bit field."] #visibility #name: #ty)
    });
    let zero_fields = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let value = super::zero(field);
        quote!(#name: #value)
    });
    let decoded_fields = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let value = super::decode(field, &quote!(raw));
        quote!(#name: #value)
    });
    let encoded_fields = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let field_name = name.to_string();
        let bit_width = super::width(field);
        let maximum = validate::bits::bit_mask(bit_width);
        let encoded = super::encode(field, &quote!(value.#name));
        quote!({
            let observed = u64::from(value.#name);
            if observed > #maximum {
                return ::core::result::Result::Err(#facade::BitsValueError::new(
                    offset, #field_name, #bit_width, observed, #maximum,
                ));
            }
            raw |= #encoded;
        })
    });
    let codec = codec::expand(
        values,
        &raw_name,
        &raw_type,
        declared_mask,
        &narrowed_raw,
        facade,
    );
    quote! {
        #[doc = "Typed raw scalar for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        #visibility struct #raw_name(#raw_type);
        impl #raw_name {
            #[doc = "Creates one typed raw scalar."]
            pub const fn new(value: #raw_type) -> Self { Self(value) }
            #[doc = "Returns the normalized raw scalar."]
            pub const fn value(self) -> #raw_type { self.0 }
        }
        impl ::core::convert::From<#raw_type> for #raw_name {
            fn from(value: #raw_type) -> Self { Self::new(value) }
        }
        impl ::core::convert::From<#raw_name> for #raw_type {
            fn from(value: #raw_name) -> Self { value.value() }
        }
        #[doc = "Owned typed value for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #values { #(#declarations,)* }
        impl #values {
            #[doc = "All-zero typed bits value."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            fn from_raw(raw: u64) -> Self { Self { #(#decoded_fields,)* } }
            fn raw(&self, offset: usize) -> ::core::result::Result<u64, #facade::BitsValueError> {
                let value = self;
                let mut raw = 0_u64;
                #(#encoded_fields)*
                ::core::result::Result::Ok(raw)
            }
        }
        impl ::core::default::Default for #values {
            fn default() -> Self { Self::ZERO }
        }
        #codec
    }
}

fn narrow_raw(raw_type: &syn::Ident, extent: usize) -> TokenStream {
    let bytes = (0..extent).map(syn::Index::from);
    quote!({
        let bytes = raw.to_le_bytes();
        #raw_type::from_le_bytes([#(bytes[#bytes]),*])
    })
}

fn marker(context: &SchemaContext<'_>) -> syn::Result<TokenStream> {
    let facade = &context.facade;
    let visibility = &context.decl.visibility;
    let marker = &context.decl.name;
    let marker_name = marker.to_string();
    let values = &context.values;
    let field_id = &context.field_id;
    let bit_width = context.bit_width;
    let extent = context.extent;
    let declared_mask = context.declared_mask;
    let raw_type = format_ident!("u{}", extent * 8);
    let scalar_kind = format_ident!("U{}", extent * 8);
    let specs = specs(context)?;
    let field_count = context.decl.fields.len();
    Ok(quote! {
        #[doc = "Marker and metadata namespace for one reusable bits schema."]
        #[derive(Clone, Copy, Debug)]
        #visibility struct #marker;
        impl #marker {
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
            type FieldId = #field_id;
            const NAME: &'static str = #marker_name;
            const BIT_WIDTH: usize = #bit_width;
            const EXTENT: usize = #extent;
            const DECLARED_MASK: u64 = #declared_mask;
            const SCALAR_KIND: #facade::ScalarKind = #facade::ScalarKind::#scalar_kind;
            const ZERO: Self::Value = #values::ZERO;
            const FIELDS: &'static [#facade::BitsFieldSpec] = &Self::FIELDS;
            fn decode_register(raw: u64) -> Self::Value { #values::from_raw(raw) }
            fn encode_register(value: &Self::Value) -> ::core::result::Result<u64, #facade::RegisterValueError> {
                value.raw(0).map_err(#facade::BitsValueError::into_register_error)
            }
            fn decode_record(bytes: &[u8], endian: #facade::Endian, offset: usize) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> {
                let raw: #raw_type = #facade::read_scalar(bytes, endian)?;
                let raw = u64::from(raw);
                let reserved = raw & !#declared_mask;
                if reserved != 0 {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved { offset, observed: reserved });
                }
                ::core::result::Result::Ok(#values::from_raw(raw))
            }
            fn encode_record(bytes: &mut [u8], endian: #facade::Endian, value: &Self::Value, offset: usize) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let raw = value.raw(offset).map_err(#facade::BitsValueError::into_schema_error)?;
                let raw = #raw_type::from_le_bytes(raw.to_le_bytes()[..#extent].try_into().map_err(|_| #facade::SchemaCodecError::NarrowingOverflow { offset, observed: raw, maximum: #declared_mask })?);
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
            let lsb = field.start.0;
            let width = super::width(field);
            Ok(quote!(#facade::BitsFieldSpec::place(
                #id, #name, #lsb, #facade::BitsFieldDefinition::new(#width),
            ).with_declaration_index(#index)))
        })
        .collect()
}
