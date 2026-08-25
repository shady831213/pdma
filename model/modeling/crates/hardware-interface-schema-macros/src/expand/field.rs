use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse2;

use crate::internal::{
    crate_path,
    syntax::{FieldDefinitionDecl, ScalarType},
};

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<FieldDefinitionDecl>(input)
        .and_then(|decl| expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &FieldDefinitionDecl) -> syn::Result<TokenStream> {
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let endian = &decl.endian;
    if !matches!(endian.to_string().as_str(), "Little" | "Big") {
        return Err(syn::Error::new(
            endian.span(),
            "endian must be Little or Big",
        ));
    }
    let scalar = scalar_tokens(decl.scalar);
    let kind = kind(decl.scalar);
    let width = scalar_width(decl.scalar);
    let raw = format_ident!("{}Raw", marker);
    let value = format_ident!("{}Value", marker);
    Ok(quote! {
        #[doc = "Typed raw bytes for one reusable scalar field definition."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #raw([u8; #width]);
        impl #raw {
            #[doc = "All-zero raw field bytes."]
            pub const ZERO: Self = Self([0; #width]);
            #[doc = "Creates typed raw field bytes."]
            pub const fn new(bytes: [u8; #width]) -> Self { Self(bytes) }
            #[doc = "Borrows the raw field bytes."]
            pub const fn as_bytes(&self) -> &[u8; #width] { &self.0 }
            #[doc = "Returns the raw field bytes."]
            pub const fn into_bytes(self) -> [u8; #width] { self.0 }
        }
        impl ::core::default::Default for #raw {
            fn default() -> Self { Self::ZERO }
        }
        impl ::core::convert::From<[u8; #width]> for #raw {
            fn from(bytes: [u8; #width]) -> Self { Self::new(bytes) }
        }
        impl ::core::convert::From<#raw> for [u8; #width] {
            fn from(raw: #raw) -> Self { raw.into_bytes() }
        }
        #[doc = "Owned semantic value for one reusable scalar field definition."]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        #visibility struct #value(#scalar);
        impl #value {
            #[doc = "Creates one semantic field value."]
            pub const fn new(value: #scalar) -> Self { Self(value) }
            #[doc = "Returns the semantic scalar."]
            pub const fn value(self) -> #scalar { self.0 }
        }
        impl ::core::convert::From<#scalar> for #value {
            fn from(value: #scalar) -> Self { Self::new(value) }
        }
        impl ::core::convert::From<#value> for #scalar {
            fn from(value: #value) -> Self { value.value() }
        }
        #[doc = "Marker and codec for a reusable scalar field definition."]
        #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Reusable scalar field definition."]
            pub const DEFINITION: #facade::FieldDefinition = #facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian);
            #[doc = "Reads the scalar from the start of a byte slice."]
            pub fn read(bytes: &[u8]) -> ::core::result::Result<#scalar, #facade::SchemaCodecError> { #facade::read_scalar(bytes, #facade::Endian::#endian) }
            #[doc = "Writes the scalar at the start of a byte slice."]
            pub fn write(bytes: &mut [u8], value: #scalar) -> ::core::result::Result<(), #facade::SchemaCodecError> { #facade::write_scalar(bytes, #facade::Endian::#endian, value) }
        }
        impl #facade::FieldSchema for #marker {
            type Scalar = #scalar;
            const DEFINITION: #facade::FieldDefinition = Self::DEFINITION;
            fn read(bytes: &[u8]) -> ::core::result::Result<Self::Scalar, #facade::SchemaCodecError> { Self::read(bytes) }
            fn write(bytes: &mut [u8], value: Self::Scalar) -> ::core::result::Result<(), #facade::SchemaCodecError> { Self::write(bytes, value) }
        }
        impl #facade::Decode for #raw {
            type Decoded = #value;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #marker::read(&self.0).map(#value::new)
            }
        }
        impl #facade::Encode for #value {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                let mut bytes = [0_u8; #width];
                #marker::write(&mut bytes, self.0)?;
                ::core::result::Result::Ok(#raw::new(bytes))
            }
        }
        impl ::core::convert::TryFrom<#raw> for #value {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw) -> ::core::result::Result<Self, Self::Error> {
                <#raw as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#value> for #raw {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #value) -> ::core::result::Result<Self, Self::Error> {
                <#value as #facade::Encode>::encode(value)
            }
        }
    })
}

fn scalar_tokens(scalar: ScalarType) -> TokenStream {
    match scalar {
        ScalarType::U8 => quote!(u8),
        ScalarType::U16 => quote!(u16),
        ScalarType::U32 => quote!(u32),
        ScalarType::U64 => quote!(u64),
    }
}

const fn scalar_width(scalar: ScalarType) -> usize {
    match scalar {
        ScalarType::U8 => 1,
        ScalarType::U16 => 2,
        ScalarType::U32 => 4,
        ScalarType::U64 => 8,
    }
}

fn kind(scalar: ScalarType) -> syn::Ident {
    format_ident!(
        "{}",
        match scalar {
            ScalarType::U8 => "U8",
            ScalarType::U16 => "U16",
            ScalarType::U32 => "U32",
            ScalarType::U64 => "U64",
        }
    )
}
