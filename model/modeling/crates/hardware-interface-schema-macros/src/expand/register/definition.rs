use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::internal::{
    crate_path,
    syntax::{AccessValue, RegisterDefinitionDecl},
    validate,
};

pub(super) fn expand(decl: &RegisterDefinitionDecl) -> syn::Result<TokenStream> {
    validate::register::check_definition(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let id_name = format_ident!("{}FieldId", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let width = decl.width.0;
    let endian = &decl.endian;
    let access = access_ident(decl.access);
    let reset_kind = &decl.reset.kind;
    let reset_value = decl.reset.value.0;
    let mask = decl.mask.0;
    let mut variants = Vec::new();
    let mut fields = Vec::new();
    let mut assertions = Vec::new();
    let mut getters = Vec::new();
    let mut setters = Vec::new();
    for field in &decl.fields {
        let method = &field.name;
        let name = method.to_string();
        let definition = &field.definition;
        let id = field.id.0;
        let lsb = field.lsb.0;
        variants.push(super::variant(&name));
        assertions.push(quote_spanned!(definition.span()=> const _: #facade::RegisterFieldDefinition = #definition::DEFINITION;));
        assertions.push(quote_spanned!(definition.span()=> const _: () = assert!(#lsb <= #width * 8 && #definition::DEFINITION.bit_width() <= #width * 8 - #lsb, "register field extends beyond register width");));
        fields.push(
            quote!(#facade::RegisterFieldSpec::place(#id, #name, #lsb, #definition::DEFINITION)),
        );
        getters.push(quote!(#[doc = "Reads the named register field."] pub fn #method(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.field_by_name(#name) }));
        let setter = format_ident!("set_{}", method);
        setters.push(quote!(#[doc = "Writes the named register field."] pub fn #setter(&mut self, value: u64) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_field_by_name(#name, value) }));
    }
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated field ID."] #item));
    Ok(quote! {
        #(#assertions)*
        #[doc = "Stable sibling-local register-field identities."] #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #id_name { #(#documented_variants),* }
        #[doc = "Marker and metadata namespace for the generated register."] #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Declared reset value."] pub const RESET_VALUE: u64 = #reset_value;
            #[doc = "Declared writable mask."] pub const WRITABLE_MASK: u64 = #mask;
            #[doc = "Complete offset-free register definition."] pub const SPEC: #facade::RegisterDefinition = #facade::RegisterDefinition::new(#marker_name, #width, #facade::Endian::#endian).with_access(#facade::Access::#access).with_reset(#facade::ResetKind::#reset_kind, #reset_value).with_writable_mask(#mask).with_fields(&[#(#fields),*]);
        }
        #[doc = "Checked immutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Creates a checked immutable register view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #(#getters)* }
        #[doc = "Checked mutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Creates a checked mutable register view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #[doc = "Writes the complete register value."] pub fn set_value(&mut self, value: u64) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_value(value) } #(#getters)* #(#setters)* }
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
