use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse2;

use crate::{
    crate_path,
    syntax::{FieldShape, RecordDecl, ScalarType},
    validate,
};

mod composable;

pub(crate) fn run(input: TokenStream) -> TokenStream {
    parse2::<RecordDecl>(input)
        .and_then(|decl| expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &RecordDecl) -> syn::Result<TokenStream> {
    if !decl.records.is_empty()
        || decl
            .fields
            .iter()
            .any(|field| matches!(field.shape, FieldShape::Reference { .. }))
    {
        return composable::expand(decl);
    }
    validate::record::check(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let id_name = format_ident!("{}FieldId", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let size = decl.size.0;
    let alignment = decl.alignment.0;
    let mut variants = Vec::new();
    let mut specs = Vec::new();
    let mut constants = Vec::new();
    let mut getters = Vec::new();
    let mut setters = Vec::new();
    for field in &decl.fields {
        let method = &field.name;
        let constant = format_ident!("{}_OFFSET", field.name.to_string().to_uppercase());
        let base = field.offset.0;
        constants.push(
            quote!(#[doc = "Declared field byte offset."] pub const #constant: usize = #base;),
        );
        let endian = &field.endian;
        match &field.shape {
            FieldShape::Scalar { scalar, id } => {
                let variant = variant(&field.name.to_string());
                let ty = scalar_tokens(*scalar);
                let kind = kind_tokens(*scalar);
                let field_name = field.name.to_string();
                let id = id.0;
                variants.push(quote!(#[doc = "Stable generated field ID."] #variant));
                specs.push(quote!(#facade::FieldSpec::new(#id, #field_name, #base, #facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian)));
                let required = base + scalar.width();
                getters.push(quote!(#[doc = "Reads the field value."] pub fn #method(&self) -> ::core::result::Result<#ty, #facade::SchemaCodecError> { let bytes = self.bytes.get(#base..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #required, actual: self.bytes.len() })?; #facade::read_scalar(bytes, #facade::Endian::#endian) }));
                let setter = format_ident!("set_{}", method);
                setters.push(quote!(#[doc = "Writes the field value."] pub fn #setter(&mut self, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> { let actual = self.bytes.len(); let bytes = self.bytes.get_mut(#base..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #required, actual })?; #facade::write_scalar(bytes, #facade::Endian::#endian, value) }));
            }
            FieldShape::Array {
                scalar,
                count,
                ids,
                stride,
            } => {
                let ty = scalar_tokens(*scalar);
                let kind = kind_tokens(*scalar);
                let count_value = count.0;
                let stride_value = stride.0;
                for (index, id) in ids.iter().enumerate() {
                    let variant = variant(&format!("{}{}", field.name, index));
                    let offset = base + index * stride_value;
                    let field_name = format!("{}[{index}]", field.name);
                    let id = id.0;
                    variants.push(quote!(#[doc = "Stable generated field ID."] #variant));
                    specs.push(quote!(#facade::FieldSpec::new(#id, #field_name, #offset, #facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian)));
                }
                getters.push(quote!(#[doc = "Reads one indexed array field value."] pub fn #method(&self, index: usize) -> ::core::result::Result<#ty, #facade::SchemaCodecError> { if index >= #count_value { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count_value, actual: index }); } let offset = index.checked_mul(#stride_value).and_then(|part| #base.checked_add(part)).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #count_value, actual: index })?; let bytes = self.bytes.get(offset..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: offset, actual: self.bytes.len() })?; #facade::read_scalar(bytes, #facade::Endian::#endian) }));
                let setter = format_ident!("set_{}", method);
                setters.push(quote!(#[doc = "Writes one indexed array field value."] pub fn #setter(&mut self, index: usize, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> { if index >= #count_value { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count_value, actual: index }); } let offset = index.checked_mul(#stride_value).and_then(|part| #base.checked_add(part)).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #count_value, actual: index })?; let actual = self.bytes.len(); let bytes = self.bytes.get_mut(offset..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: offset, actual })?; #facade::write_scalar(bytes, #facade::Endian::#endian, value) }));
            }
            FieldShape::Reference { definition, .. } => {
                return Err(syn::Error::new_spanned(
                    definition,
                    "referenced fields require composable record expansion",
                ));
            }
        }
    }
    Ok(quote! {
        #[doc = "Stable field identities for the generated record."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #id_name { #(#variants),* }
        #[doc = "Marker and metadata namespace for the generated record."]
        #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker { #(#constants)* #[doc = "Complete fixed-record specification."] pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::new(#size, #alignment, &[#(#specs),*]); }
        #[doc = "Checked immutable byte view for the generated record."]
        #[derive(Debug, PartialEq, Eq)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Checks the byte extent and creates a view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* }
        #[doc = "Checked mutable byte view for the generated record."]
        #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Checks the byte extent and creates a mutable view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* #(#setters)* }
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
fn kind_tokens(scalar: ScalarType) -> syn::Ident {
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
fn variant(name: &str) -> syn::Ident {
    format_ident!(
        "{}",
        name.split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            })
            .collect::<String>()
    )
}
