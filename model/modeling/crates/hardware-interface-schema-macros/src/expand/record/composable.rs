use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{Path, spanned::Spanned};

use crate::internal::{
    crate_path,
    syntax::{FieldShape, RecordDecl, RecordPlacementDecl, ScalarType},
    validate,
};

pub(super) fn expand(decl: &RecordDecl) -> syn::Result<TokenStream> {
    validate::record::check(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let id_name = format_ident!("{}FieldId", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let size = decl.size.0;
    let alignment = decl.alignment.0;
    let value_items = super::values::expand(decl, &facade);
    let FieldExpansion {
        mut variants,
        specs,
        mut constants,
        getters,
        setters,
        mut assertions,
    } = expand_fields(decl, &facade);
    let mut placements = Vec::new();
    let mut nested_getters = Vec::new();
    let mut nested_mut_getters = Vec::new();
    for record in &decl.records {
        let method = &record.name;
        let placement_name = method.to_string();
        let definition = &record.definition;
        let id = record.id.0;
        let offset = record.offset.0;
        variants.push(variant(&placement_name));
        let upper = placement_name.to_uppercase();
        let offset_constant = format_ident!("{upper}_OFFSET");
        let definition_constant = format_ident!("{upper}_DEFINITION");
        constants.push(quote!(#[doc = "Nested record byte offset."] pub const #offset_constant: usize = #offset;));
        constants.push(quote!(#[doc = "Referenced nested record definition."] pub const #definition_constant: #facade::RecordSpec = #definition::SPEC;));
        assertions.push(
            quote_spanned!(definition.span()=> const _: #facade::RecordSpec = #definition::SPEC;),
        );
        assertions.extend(nested_layout_assertions(decl, record));
        placements.push(quote!(#facade::RecordPlacement::new(#id, #placement_name, #offset, &#definition::SPEC)));
        let child_view = suffixed_path(definition, "View");
        let child_view_mut = suffixed_path(definition, "ViewMut");
        nested_getters.push(quote!(#[doc = "Returns a checked immutable nested record view."] pub fn #method(&self) -> ::core::result::Result<#child_view<'_>, #facade::SchemaCodecError> { let required = #offset.checked_add(#definition::SPEC.size()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let bytes = self.bytes.get(#offset..required).ok_or(#facade::SchemaCodecError::BufferTooShort { required, actual: self.bytes.len() })?; #child_view::new(bytes) }));
        let mutable = format_ident!("{}_mut", method);
        nested_mut_getters.push(quote!(#[doc = "Returns a checked mutable nested record view."] pub fn #mutable(&mut self) -> ::core::result::Result<#child_view_mut<'_>, #facade::SchemaCodecError> { let required = #offset.checked_add(#definition::SPEC.size()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let actual = self.bytes.len(); let bytes = self.bytes.get_mut(#offset..required).ok_or(#facade::SchemaCodecError::BufferTooShort { required, actual })?; #child_view_mut::new(bytes) }));
    }
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated placement ID."] #item));
    Ok(quote! {
        #(#assertions)*
        #[doc = "Stable sibling-local identities for the generated record."] #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #id_name { #(#documented_variants),* }
        #[doc = "Marker and metadata namespace for the generated record."] #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker { #(#constants)* #[doc = "Complete named record definition."] pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(#marker_name, #size, #alignment, &[#(#specs),*], &[#(#placements),*]); }
        #[doc = "Checked immutable byte view for the generated record."] #[derive(Debug, PartialEq, Eq)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Creates a checked immutable view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* #(#nested_getters)* }
        #[doc = "Checked mutable byte view for the generated record."] #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Creates a checked mutable view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* #(#setters)* #(#nested_getters)* #(#nested_mut_getters)* }
        #value_items
    })
}

fn nested_layout_assertions(decl: &RecordDecl, record: &RecordPlacementDecl) -> Vec<TokenStream> {
    let child_offset = record.offset.0;
    let child_definition = &record.definition;
    let parent_size = decl.size.0;
    let span = record.offset.1;
    let mut output = vec![
        quote_spanned!(span=> const _: () = assert!(#child_offset <= #parent_size && #child_definition::SPEC.size() <= #parent_size - #child_offset, "nested record placement extends beyond its containing extent");),
    ];
    let overlap = |field_offset: usize, field_width: TokenStream| {
        quote_spanned!(span=> const _: () = {
            let child_width = #child_definition::SPEC.size();
            let field_width = #field_width;
            if #child_offset <= #parent_size
                && child_width <= #parent_size - #child_offset
                && #field_offset <= #parent_size
                && field_width <= #parent_size - #field_offset
            {
                assert!(
                    #child_offset >= #field_offset + field_width
                        || #field_offset >= #child_offset + child_width,
                    "nested record placement overlaps a scalar placement"
                );
            }
        };)
    };
    for field in &decl.fields {
        match &field.shape {
            FieldShape::Scalar { scalar, .. } => {
                let width = scalar.width();
                output.push(overlap(field.offset.0, quote!(#width)));
            }
            FieldShape::Reference { definition, .. } => {
                output.push(overlap(
                    field.offset.0,
                    quote!(#definition::DEFINITION.width()),
                ));
            }
            FieldShape::Array {
                scalar,
                ids,
                stride,
                ..
            } => {
                let width = scalar.width();
                for index in 0..ids.len() {
                    let offset = field.offset.0 + index * stride.0;
                    output.push(overlap(offset, quote!(#width)));
                }
            }
        }
    }
    output
}

struct FieldExpansion {
    variants: Vec<syn::Ident>,
    specs: Vec<TokenStream>,
    constants: Vec<TokenStream>,
    getters: Vec<TokenStream>,
    setters: Vec<TokenStream>,
    assertions: Vec<TokenStream>,
}

fn expand_fields(decl: &RecordDecl, facade: &TokenStream) -> FieldExpansion {
    let mut output = FieldExpansion {
        variants: Vec::new(),
        specs: Vec::new(),
        constants: Vec::new(),
        getters: Vec::new(),
        setters: Vec::new(),
        assertions: Vec::new(),
    };
    for field in &decl.fields {
        let method = &field.name;
        let field_name = method.to_string();
        let upper = field_name.to_uppercase();
        let offset_constant = format_ident!("{upper}_OFFSET");
        let definition_constant = format_ident!("{upper}_DEFINITION");
        let base = field.offset.0;
        output.constants.push(quote!(#[doc = "Declared field byte offset."] pub const #offset_constant: usize = #base;));
        match &field.shape {
            FieldShape::Scalar { scalar, id } => {
                let ty = scalar_tokens(*scalar);
                let kind = kind_tokens(*scalar);
                let endian = &field.endian;
                let numeric_id = id.0;
                output.variants.push(variant(&field_name));
                output.constants.push(quote!(#[doc = "Placed scalar definition."] pub const #definition_constant: #facade::FieldDefinition = #facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian);));
                output.specs.push(quote!(#facade::FieldSpec::place(#numeric_id, #field_name, #base, Self::#definition_constant)));
                let setter = format_ident!("set_{}", method);
                let required = base + scalar.width();
                output.getters.push(quote!(#[doc = "Reads the placed scalar field."] pub fn #method(&self) -> ::core::result::Result<#ty, #facade::SchemaCodecError> { let bytes = self.bytes.get(#base..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #required, actual: self.bytes.len() })?; #facade::read_scalar(bytes, #facade::Endian::#endian) }));
                output.setters.push(quote!(#[doc = "Writes the placed scalar field."] pub fn #setter(&mut self, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> { let actual = self.bytes.len(); let bytes = self.bytes.get_mut(#base..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #required, actual })?; #facade::write_scalar(bytes, #facade::Endian::#endian, value) }));
            }
            FieldShape::Reference { definition, id } => {
                let numeric_id = id.0;
                output.variants.push(variant(&field_name));
                output.assertions.push(quote_spanned!(definition.span()=> const _: #facade::FieldDefinition = #definition::DEFINITION;));
                output.constants.push(quote!(#[doc = "Referenced scalar definition."] pub const #definition_constant: #facade::FieldDefinition = #definition::DEFINITION;));
                output.specs.push(quote!(#facade::FieldSpec::place(#numeric_id, #field_name, #base, Self::#definition_constant)));
                let ty = quote!(<#definition as #facade::FieldSchema>::Scalar);
                let setter = format_ident!("set_{}", method);
                output.getters.push(quote!(#[doc = "Reads the placed reusable scalar field."] pub fn #method(&self) -> ::core::result::Result<#ty, #facade::SchemaCodecError> { let required = #base.checked_add(#definition::DEFINITION.width()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let bytes = self.bytes.get(#base..required).ok_or(#facade::SchemaCodecError::BufferTooShort { required, actual: self.bytes.len() })?; <#definition as #facade::FieldSchema>::read(bytes) }));
                output.setters.push(quote!(#[doc = "Writes the placed reusable scalar field."] pub fn #setter(&mut self, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> { let required = #base.checked_add(#definition::DEFINITION.width()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let actual = self.bytes.len(); let bytes = self.bytes.get_mut(#base..required).ok_or(#facade::SchemaCodecError::BufferTooShort { required, actual })?; <#definition as #facade::FieldSchema>::write(bytes, value) }));
            }
            FieldShape::Array {
                scalar,
                count,
                ids,
                stride,
            } => {
                let ty = scalar_tokens(*scalar);
                let kind = kind_tokens(*scalar);
                let endian = &field.endian;
                let count = count.0;
                let stride = stride.0;
                output.constants.push(quote!(#[doc = "Placed array element definition."] pub const #definition_constant: #facade::FieldDefinition = #facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian);));
                for (index, id) in ids.iter().enumerate() {
                    output
                        .variants
                        .push(variant(&format!("{field_name}{index}")));
                    let name = format!("{field_name}[{index}]");
                    let offset = base + index * stride;
                    let id = id.0;
                    output.specs.push(quote!(#facade::FieldSpec::place(#id, #name, #offset, Self::#definition_constant)));
                }
                let setter = format_ident!("set_{}", method);
                output.getters.push(quote!(#[doc = "Reads one indexed array field."] pub fn #method(&self, index: usize) -> ::core::result::Result<#ty, #facade::SchemaCodecError> { if index >= #count { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: index }); } let offset = index.checked_mul(#stride).and_then(|part| #base.checked_add(part)).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: index })?; let bytes = self.bytes.get(offset..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: offset, actual: self.bytes.len() })?; #facade::read_scalar(bytes, #facade::Endian::#endian) }));
                output.setters.push(quote!(#[doc = "Writes one indexed array field."] pub fn #setter(&mut self, index: usize, value: #ty) -> ::core::result::Result<(), #facade::SchemaCodecError> { if index >= #count { return Err(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: index }); } let offset = index.checked_mul(#stride).and_then(|part| #base.checked_add(part)).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #count, actual: index })?; let actual = self.bytes.len(); let bytes = self.bytes.get_mut(offset..).ok_or(#facade::SchemaCodecError::BufferTooShort { required: offset, actual })?; #facade::write_scalar(bytes, #facade::Endian::#endian, value) }));
            }
        }
    }
    output
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
fn suffixed_path(path: &Path, suffix: &str) -> Path {
    let mut output = path.clone();
    if let Some(last) = output.segments.last_mut() {
        last.ident = format_ident!("{}{}", last.ident, suffix);
    }
    output
}
