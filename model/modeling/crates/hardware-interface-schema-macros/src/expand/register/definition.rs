use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{
    crate_path,
    syntax::{AccessValue, RegisterDefinitionDecl, RegisterFieldsDecl},
    validate,
};

struct FieldsExpansion {
    id_declaration: TokenStream,
    id_type: TokenStream,
    specs: TokenStream,
    getters: Vec<TokenStream>,
    setters: Vec<TokenStream>,
    assertions: TokenStream,
}

pub(super) fn expand(decl: &RegisterDefinitionDecl) -> syn::Result<TokenStream> {
    validate::register::check_definition(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let width = decl.width.0;
    let alignment = decl.alignment.map_or(width, |alignment| alignment.0);
    let scalar = format_ident!("u{}", width * 8);
    let endian = &decl.endian;
    let access = access_ident(decl.access);
    let reset_kind = &decl.reset.kind;
    let reset_value = decl.reset.value.0;
    let mask = decl.mask.0;
    let fields = expand_fields(decl, &facade)?;
    let id_declaration = fields.id_declaration;
    let id_type = fields.id_type;
    let field_specs = fields.specs;
    let getters = fields.getters;
    let setters = fields.setters;
    let assertions = fields.assertions;
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
        #[doc = "Checked immutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Creates a checked immutable register view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #(#getters)* }
        #[doc = "Checked mutable byte view for the generated register."] #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Creates a checked mutable register view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::RegisterCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#width).ok_or(#facade::RegisterCodecError::BufferTooShort { required: #width, actual })?; Ok(Self { bytes }) } #[doc = "Reads the complete register value."] pub fn value(&self) -> ::core::result::Result<u64, #facade::RegisterCodecError> { #marker::SPEC.view(self.bytes)?.value() } #[doc = "Writes the complete register value."] pub fn set_value(&mut self, value: u64) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_value(value) } #(#getters)* #(#setters)* }
        impl #facade::RegisterLayout for #marker {
            type Value = #scalar;
            type View<'a> = #view_name<'a>;
            type ViewMut<'a> = #view_mut_name<'a>;
            const NAME: &'static str = #marker_name;
            const ALIGNMENT: usize = #alignment;
            const EXTENT: usize = #width;
            const ZERO: Self::Value = 0;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> { #facade::read_scalar(bytes, #facade::Endian::#endian) }
            fn encode(bytes: &mut [u8], value: &Self::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> { #facade::write_scalar(bytes, #facade::Endian::#endian, *value) }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#width).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #width, actual })?; Ok(#view_name { bytes }) }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#width).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #width, actual })?; Ok(#view_mut_name { bytes }) }
        }
    })
}

fn expand_fields(
    decl: &RegisterDefinitionDecl,
    facade: &TokenStream,
) -> syn::Result<FieldsExpansion> {
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let id_name = format_ident!("{}FieldId", marker);
    match &decl.fields {
        RegisterFieldsDecl::Inline(declared) => {
            expand_inline_fields(declared, visibility, marker, &id_name, facade)
        }
        RegisterFieldsDecl::Reference(path) => {
            let width_bits = decl.width.0 * 8;
            let writable_mask = decl.mask.0;
            let getters = vec![quote! {
                #[doc = "Decodes the reusable typed bits value."]
                pub fn fields(&self) -> ::core::result::Result<<#path as #facade::BitsLayout>::Value, #facade::RegisterCodecError> {
                    let raw = #marker::SPEC.view(self.bytes)?.value()?;
                    ::core::result::Result::Ok(<#path as #facade::BitsLayout>::decode_register(raw))
                }
            }];
            let setters = vec![quote! {
                #[doc = "Encodes the reusable typed bits value while preserving undeclared bits."]
                pub fn set_fields(&mut self, value: <#path as #facade::BitsLayout>::Value) -> ::core::result::Result<(), #facade::RegisterCodecError> {
                    let encoded = <#path as #facade::BitsLayout>::encode_register(&value)?;
                    let declared_mask = <#path as #facade::BitsLayout>::DECLARED_MASK;
                    let mut view = #marker::SPEC.view_mut(self.bytes)?;
                    let current = view.value()?;
                    view.set_value((current & !declared_mask) | encoded)
                }
            }];
            Ok(FieldsExpansion {
                id_declaration: quote!(
                    #[doc = "Stable field identity reused from the referenced bits schema."]
                    #visibility type #id_name = <#path as #facade::BitsLayout>::FieldId;
                ),
                id_type: quote!(u16),
                specs: quote!(<#path as #facade::BitsLayout>::FIELDS),
                getters,
                setters,
                assertions: quote! {
                    const _: () = assert!(#path::BIT_WIDTH <= #width_bits, "bits schema exceeds register width");
                    const _: () = assert!(#writable_mask & !#path::DECLARED_MASK == 0, "writable mask contains undeclared bits");
                },
            })
        }
    }
}

fn expand_inline_fields(
    declared: &[crate::internal::syntax::BitFieldDecl],
    visibility: &syn::Visibility,
    marker: &syn::Ident,
    id_name: &syn::Ident,
    facade: &TokenStream,
) -> syn::Result<FieldsExpansion> {
    let mut variants = Vec::new();
    let mut fields = Vec::new();
    let mut getters = Vec::new();
    let mut setters = Vec::new();
    for (index, field) in declared.iter().enumerate() {
        let method = &field.name;
        let name = method.to_string();
        let id = u16::try_from(index.saturating_add(1))
            .map_err(|_| syn::Error::new(method.span(), "register field ID exceeds u16"))?;
        let lsb = field.start.0;
        let bit_width = crate::internal::expand::bits::width(field);
        let semantic_type = crate::internal::expand::bits::semantic_type(field);
        variants.push(super::variant(&name));
        fields.push(
            quote!(#facade::BitsFieldSpec::place(#id, #name, #lsb, #facade::BitsFieldDefinition::new(#bit_width)).with_declaration_index(#index)),
        );
        let raw = quote!(raw);
        let decoded = crate::internal::expand::bits::decode_local(field, &raw);
        getters.push(quote!(#[doc = "Reads the named register field."] pub fn #method(&self) -> ::core::result::Result<#semantic_type, #facade::RegisterCodecError> { let raw = #marker::SPEC.view(self.bytes)?.field_by_name(#name)?; Ok(#decoded) }));
        let setter = format_ident!("set_{}", method);
        let value = quote!(value);
        let encoded = crate::internal::expand::bits::encode_local(&value);
        setters.push(quote!(#[doc = "Writes the named register field."] pub fn #setter(&mut self, value: #semantic_type) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_field_by_name(#name, #encoded) }));
    }
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated field ID."] #item));
    Ok(FieldsExpansion {
        id_declaration: quote! {
            #[doc = "Stable sibling-local register-field identities."]
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #visibility enum #id_name { #(#documented_variants),* }
        },
        id_type: quote!(u16),
        specs: quote!(&[#(#fields),*]),
        getters,
        setters,
        assertions: TokenStream::new(),
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
