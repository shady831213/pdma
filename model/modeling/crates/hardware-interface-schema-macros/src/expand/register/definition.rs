use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{
    crate_path,
    syntax::{AccessValue, RegisterDefinitionDecl},
    validate,
};

mod fields;
mod owned;

pub(super) fn expand(decl: &RegisterDefinitionDecl) -> syn::Result<TokenStream> {
    validate::register::check_definition(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let values_name = format_ident!("{}Values", marker);
    let raw_name = format_ident!("{}Raw", marker);
    let width = decl.width.0;
    let alignment = decl.alignment.map_or(width, |alignment| alignment.0);
    let scalar = format_ident!("u{}", width * 8);
    let endian = &decl.endian;
    let access = access_ident(decl.access);
    let reset_kind = &decl.reset.kind;
    let reset_value = decl.reset.value.0;
    let mask = decl.mask.0;
    let fields = fields::expand(decl, &facade)?;
    let id_declaration = fields.id_declaration;
    let id_type = fields.id_type;
    let field_specs = fields.specs;
    let getters = fields.getters;
    let setters = fields.setters;
    let value_methods = fields.value_methods;
    let assertions = fields.assertions;
    let reset_scalar = fields::scalar_from_u64(&scalar, width, &quote!(#reset_value));
    let reset_bytes = if endian == "Little" {
        quote!((#reset_scalar).to_le_bytes())
    } else {
        quote!((#reset_scalar).to_be_bytes())
    };
    let owned = owned::expand(owned::Context {
        visibility,
        values: &values_name,
        raw: &raw_name,
        view: &view_name,
        scalar: &scalar,
        width,
        reset_scalar: &reset_scalar,
        reset_bytes: &reset_bytes,
        endian,
        value_methods: &value_methods,
        facade: &facade,
    });
    Ok(quote! {
        #id_declaration
        #assertions
        #[doc = "Marker and metadata namespace for the generated register."] #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Required byte alignment for placements of this register."] pub const ALIGNMENT: usize = #alignment;
            #[doc = "Encoded byte extent of this register."] pub const EXTENT: usize = #width;
            #[doc = "Declared reset value."] pub const RESET_VALUE: u64 = #reset_value;
            #[doc = "Declared writable mask."] pub const WRITABLE_MASK: u64 = #mask;
            #[doc = "Complete offset-free register definition."] pub const SPEC: #facade::RegisterDefinition<#id_type> = #facade::RegisterDefinition::<#id_type>::new(#marker_name, #width, #facade::Endian::#endian).with_alignment(#alignment).with_access(#facade::Access::#access).with_reset(#facade::ResetKind::#reset_kind, #reset_value).with_writable_mask(#mask).with_fields(#field_specs);
        }
        #owned
        #[doc = "Checked immutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Creates a checked immutable register view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #(#getters)* }
        #[doc = "Checked mutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Creates a checked mutable register view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #[doc = "Writes the complete register value."] pub fn set_value(&mut self, value: u64) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_value(value) } #(#getters)* #(#setters)* }
        impl #facade::RegisterLayout for #marker {
            type Value = #values_name;
            type View<'a> = #view_name<'a>;
            type ViewMut<'a> = #view_mut_name<'a>;
            const NAME: &'static str = #marker_name;
            const ALIGNMENT: usize = #alignment;
            const EXTENT: usize = #width;
            const ZERO: Self::Value = #values_name::ZERO;
            const DEFAULT: Self::Value = #values_name::RESET;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> { #facade::read_scalar(bytes, #facade::Endian::#endian).map(#values_name::new) }
            fn encode(bytes: &mut [u8], value: &Self::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> { #facade::write_scalar(bytes, #facade::Endian::#endian, value.raw()) }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#width).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #width, actual })?; Ok(#view_name { bytes }) }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#width).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #width, actual })?; Ok(#view_mut_name { bytes }) }
        }
    })
}

fn access_ident(access: AccessValue) -> syn::Ident {
    format_ident!(
        "{}",
        match access {
            AccessValue::ReadOnly => "ReadOnly",
            AccessValue::ReadWrite => "ReadWrite",
            AccessValue::WriteOnly => "WriteOnly",
        }
    )
}
