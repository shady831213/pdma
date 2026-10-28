use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::validate;

use super::SchemaContext;

pub(super) fn expand(context: &SchemaContext<'_>) -> TokenStream {
    let facade = &context.facade;
    let rust_docs = context.decl.documentation.rust_docs();
    let visibility = &context.decl.visibility;
    let marker = &context.decl.name;
    let values = &context.values;
    let raw_name = format_ident!("{}Raw", context.decl.name);
    let raw_type = format_ident!("u{}", context.extent * 8);
    let declared_mask = context.declared_mask;
    let narrowed_raw = narrow_raw(&raw_type, context.extent);
    let declarations = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let ty = super::super::field_type(marker, field);
        let rust_docs = field.documentation.rust_docs();
        quote!(#rust_docs #[doc = "Generated typed bit field."] #visibility #name: #ty)
    });
    let constructor_parameters = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let ty = super::super::field_type(marker, field);
        quote!(#name: #ty)
    });
    let constructor_fields = context.decl.fields.iter().map(|field| &field.name);
    let accessors = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let ty = super::super::field_type(marker, field);
        let rust_docs = field.documentation.rust_docs();
        quote! {
            #rust_docs
            #[doc = "Returns one generated bit-field value."]
            pub const fn #name(self) -> #ty { self.#name }
        }
    });
    let zero_fields = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let value = super::super::field_zero(marker, field);
        quote!(#name: #value)
    });
    let decoded_fields = context.decl.fields.iter().map(|field| {
        let name = &field.name;
        let value = super::super::field_decode(
            marker,
            field,
            &quote!(raw),
            &quote!(#facade::SchemaCoordinate::Byte(0)),
        );
        quote!(#name: #value)
    });
    let encoded_fields = encoded_fields(context);
    let codec = super::codec::expand(&super::codec::Context {
        values,
        raw_name: &raw_name,
        raw_type: &raw_type,
        declared_mask,
        narrowed_raw: &narrowed_raw,
        facade,
        marker,
    });
    let transparent_conversions = transparent_conversions(context);
    quote! {
        #[doc = "Typed raw scalar for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, ::core::default::Default, Eq, PartialEq)]
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
        #rust_docs
        #[doc = "Owned typed value for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #values { #(#declarations,)* }
        impl #values {
            #[doc = "Creates one typed bits value."]
            pub const fn new(#(#constructor_parameters),*) -> Self { Self { #(#constructor_fields,)* } }
            #[doc = "Semantic default with zero integers and first-declared enum variants."]
            pub const ZERO: Self = Self { #(#zero_fields,)* };
            #(#accessors)*
            fn from_raw(raw: u64) -> ::core::result::Result<Self, #facade::SchemaCodecError> { ::core::result::Result::Ok(Self { #(#decoded_fields,)* }) }
            fn raw(&self) -> ::core::result::Result<u64, #facade::BitsValueError> {
                let value = self;
                let mut raw = 0_u64;
                #(#encoded_fields)*
                ::core::result::Result::Ok(raw)
            }
        }
        impl ::core::default::Default for #values {
            fn default() -> Self { Self::ZERO }
        }
        #transparent_conversions
        #codec
    }
}

fn encoded_fields(context: &SchemaContext<'_>) -> Vec<TokenStream> {
    let facade = &context.facade;
    let marker = &context.decl.name;
    context
        .decl
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let identity = format_ident!("{}_IDENTITY", name.to_string().to_ascii_uppercase());
            let lsb = field.start.0;
            let bit_width = super::super::width(field);
            let maximum = validate::bits::bit_mask(bit_width);
            let encoded = super::super::encode(field, &quote!(value.#name));
            let observed = super::super::field_raw(field, &quote!(value.#name));
            quote!({
                let observed = #observed;
                if observed > #maximum {
                    return ::core::result::Result::Err(#facade::BitsValueError::new(
                        #marker::#identity, #lsb, #bit_width, observed, #maximum,
                    ));
                }
                raw |= #encoded;
            })
        })
        .collect()
}

fn transparent_conversions(context: &SchemaContext<'_>) -> Option<TokenStream> {
    let field = context.decl.fields.first()?;
    if context.decl.fields.len() != 1 {
        return None;
    }
    let values = &context.values;
    let name = &field.name;
    let ty = super::super::field_type(&context.decl.name, field);
    Some(quote! {
        impl ::core::convert::From<#ty> for #values {
            fn from(value: #ty) -> Self { Self::new(value) }
        }
        impl ::core::convert::From<#values> for #ty {
            fn from(value: #values) -> Self { value.#name() }
        }
    })
}

pub(super) fn narrow_raw(raw_type: &syn::Ident, extent: usize) -> TokenStream {
    let indexes = (0..extent).map(syn::Index::from);
    quote!({
        let raw_bytes = raw.to_le_bytes();
        #raw_type::from_le_bytes([#(raw_bytes[#indexes]),*])
    })
}
