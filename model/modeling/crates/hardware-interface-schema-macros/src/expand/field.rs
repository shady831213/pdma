use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse2;

use crate::{
    crate_path,
    syntax::{FieldDefinitionDecl, ScalarType},
};

pub(crate) fn run(input: TokenStream) -> TokenStream {
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
    Ok(quote! {
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
