use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{Path, spanned::Spanned};

use crate::internal::{
    crate_path,
    expand::register::variant,
    syntax::{MapDecl, RegisterPlacementDecl},
};

pub(super) fn expand(decl: &MapDecl) -> syn::Result<TokenStream> {
    validate(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let id_name = format_ident!("{}RegisterId", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let size = decl.size.0;
    let mut variants = Vec::new();
    let mut fixed_specs = Vec::new();
    let mut bank_specs = Vec::new();
    let mut match_arms = Vec::new();
    let mut assertions = Vec::new();
    let mut getters = Vec::new();
    let mut mutable_getters = Vec::new();
    for register in &decl.referenced_fixed {
        let item_variant = variant(&register.symbol.to_string());
        let id = quote!(#id_name::#item_variant);
        let spec = placement(register, &id, &facade);
        variants.push(item_variant.clone());
        fixed_specs.push(spec.clone());
        match_arms.push(quote!(#id_name::#item_variant => #spec));
        let offset = register.offset.0;
        let definition = &register.definition;
        let span = definition.span();
        assertions
            .push(quote_spanned!(span=> const _: #facade::RegisterDefinition = #definition::SPEC;));
        assertions.push(quote_spanned!(span=> const _: () = assert!(#offset <= #size && #definition::SPEC.width() <= #size - #offset, "register placement extends beyond its containing extent");));
        let (getter, mutable) = accessor_tokens(
            &register.symbol,
            definition,
            &AccessorContext {
                source: AccessSource::Fixed(offset),
                facade: &facade,
            },
        );
        getters.push(getter);
        mutable_getters.push(mutable);
    }
    for bank in &decl.banks {
        let bank_name = bank.name.to_string();
        let count = bank.count.0;
        let base = bank.base.0;
        let stride = bank.stride.0;
        let mut members = Vec::new();
        for register in &bank.referenced {
            let item_variant = variant(&format!("{}_{}", bank.name, register.symbol));
            let id = quote!(#id_name::#item_variant);
            let spec = placement(register, &id, &facade);
            variants.push(item_variant.clone());
            members.push(spec.clone());
            match_arms.push(quote!(#id_name::#item_variant => #spec));
            let relative = register.offset.0;
            let definition = &register.definition;
            let span = definition.span();
            assertions.push(
                quote_spanned!(span=> const _: #facade::RegisterDefinition = #definition::SPEC;),
            );
            assertions.push(quote_spanned!(span=> const _: () = assert!(#relative <= #stride && #definition::SPEC.width() <= #stride - #relative, "register placement extends beyond its containing extent");));
            assertions.push(quote_spanned!(span=> const _: () = assert!(#count == 0 || (#base <= #size && #relative <= #size - #base && (#count - 1) <= (#size - #base - #relative) / #stride && #definition::SPEC.width() <= #size - (#base + (#count - 1) * #stride + #relative)), "expanded register placement extends beyond map size");));
            let method = format_ident!("{}_{}", bank.name, register.symbol);
            let (getter, mutable) = accessor_tokens(
                &method,
                definition,
                &AccessorContext {
                    source: AccessSource::Repeated {
                        count,
                        base,
                        stride,
                        relative,
                    },
                    facade: &facade,
                },
            );
            getters.push(getter);
            mutable_getters.push(mutable);
        }
        bank_specs.push(quote!(#facade::RepeatedRegisterBank::new(#bank_name, #base, #count, #stride, &[#(#members),*])));
    }
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated register placement ID."] #item));
    Ok(quote! {
        #(#assertions)*
        #[doc = "Stable placement-local register identities."] #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #id_name { #(#documented_variants),* }
        #[doc = "Marker and metadata namespace for the generated register map."] #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Complete named register map."] pub const SPEC: #facade::RegisterMapSpec<#id_name> = #facade::RegisterMapSpec::named(#marker_name, #size, &[#(#fixed_specs),*], &[#(#bank_specs),*]);
            #[doc = "Returns one relative typed register placement."] pub const fn register(id: #id_name) -> #facade::RegisterSpec<#id_name> { match id { #(#match_arms),* } }
            #[doc = "Locates an exact expanded register start."] pub fn locate(offset: usize) -> ::core::option::Option<#facade::RegisterLocation<#id_name>> { Self::SPEC.locate(offset) }
            #[doc = "Validates and resolves the complete map."] pub fn resolve() -> ::core::result::Result<#facade::ResolvedRegisterMap<#id_name>, #facade::RegisterMapError<#id_name>> { Self::SPEC.resolve() }
        }
        #[doc = "Checked immutable byte view for the generated register map."] #[derive(Debug)] #visibility struct #view_name<'a> { bytes: &'a [u8] }
        impl<'a> #view_name<'a> { #[doc = "Creates a checked immutable map view."] pub fn new(bytes: &'a [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* }
        #[doc = "Checked mutable byte view for the generated register map."] #[derive(Debug)] #visibility struct #view_mut_name<'a> { bytes: &'a mut [u8] }
        impl<'a> #view_mut_name<'a> { #[doc = "Creates a checked mutable map view."] pub fn new(bytes: &'a mut [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { let actual = bytes.len(); let bytes = bytes.get_mut(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?; Ok(Self { bytes }) } #(#getters)* #(#mutable_getters)* }
    })
}

fn validate(decl: &MapDecl) -> syn::Result<()> {
    if !decl.fixed.is_empty() || decl.banks.iter().any(|bank| !bank.registers.is_empty()) {
        return Err(syn::Error::new(
            decl.name.span(),
            "inline and referenced register placements cannot be mixed",
        ));
    }
    if !decl.fixed_groups.is_empty() {
        return Err(syn::Error::new(
            decl.name.span(),
            "fixed groups cannot be mixed with referenced register placements",
        ));
    }
    let registers = decl
        .referenced_fixed
        .iter()
        .chain(decl.banks.iter().flat_map(|bank| &bank.referenced));
    let mut ids = HashSet::new();
    for register in registers {
        if !ids.insert(register.id.0) {
            return Err(syn::Error::new(
                register.id.1,
                "duplicate register placement ID",
            ));
        }
    }
    let mut generated = HashSet::new();
    let mut names = HashSet::new();
    for register in &decl.referenced_fixed {
        generated.insert(variant(&register.symbol.to_string()).to_string());
        if !names.insert(register.name.value()) {
            return Err(syn::Error::new(
                register.name.span(),
                "duplicate fixed register placement name",
            ));
        }
    }
    let mut bank_names = HashSet::new();
    for bank in &decl.banks {
        if !bank_names.insert(bank.name.to_string()) {
            return Err(syn::Error::new(
                bank.name.span(),
                "duplicate register bank name",
            ));
        }
        if bank.stride.0 == 0 {
            return Err(syn::Error::new(
                bank.stride.1,
                "bank stride must be nonzero",
            ));
        }
        let mut names = HashSet::new();
        for register in &bank.referenced {
            let symbol = variant(&format!("{}_{}", bank.name, register.symbol)).to_string();
            if !generated.insert(symbol) {
                return Err(syn::Error::new(
                    register.symbol.span(),
                    "generated register placement identifier collision",
                ));
            }
            if !names.insert(register.name.value()) {
                return Err(syn::Error::new(
                    register.name.span(),
                    "duplicate bank member placement name",
                ));
            }
        }
    }
    Ok(())
}

fn placement(
    register: &RegisterPlacementDecl,
    id: &TokenStream,
    facade: &TokenStream,
) -> TokenStream {
    let definition = &register.definition;
    let numeric_id = register.id.0;
    let name = &register.name;
    let offset = register.offset.0;
    quote!(#facade::RegisterSpec::place(#id, #name, #offset, #definition::SPEC).with_numeric_id(#numeric_id))
}

enum AccessSource {
    Fixed(usize),
    Repeated {
        count: usize,
        base: usize,
        stride: usize,
        relative: usize,
    },
}

struct AccessorContext<'a> {
    source: AccessSource,
    facade: &'a TokenStream,
}

fn accessor_tokens(
    method: &syn::Ident,
    definition: &Path,
    context: &AccessorContext<'_>,
) -> (TokenStream, TokenStream) {
    let facade = context.facade;
    let view = suffixed_path(definition, "View");
    let view_mut = suffixed_path(definition, "ViewMut");
    let mutable_method = format_ident!("{}_mut", method);
    let (parameters, offset_expression, index_check) = match &context.source {
        AccessSource::Fixed(offset) => (
            quote!(),
            quote!(::core::option::Option::Some(#offset)),
            quote!(),
        ),
        AccessSource::Repeated {
            count,
            base,
            stride,
            relative,
        } => (
            quote!(instance: usize),
            quote!(instance.checked_mul(#stride).and_then(|part| #base.checked_add(part)).and_then(|start| start.checked_add(#relative))),
            quote!(if instance >= #count { return Err(#facade::RegisterCodecError::BufferTooShort { required: #count, actual: instance }); }),
        ),
    };
    let getter = quote!(#[doc = "Returns a checked immutable placed register view."] pub fn #method(&self, #parameters) -> ::core::result::Result<#view<'_>, #facade::RegisterCodecError> { #index_check let offset = #offset_expression.ok_or(#facade::RegisterCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let required = offset.checked_add(#definition::SPEC.width()).ok_or(#facade::RegisterCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let bytes = self.bytes.get(offset..required).ok_or(#facade::RegisterCodecError::BufferTooShort { required, actual: self.bytes.len() })?; #view::new(bytes) });
    let mutable = quote!(#[doc = "Returns a checked mutable placed register view."] pub fn #mutable_method(&mut self, #parameters) -> ::core::result::Result<#view_mut<'_>, #facade::RegisterCodecError> { #index_check let offset = #offset_expression.ok_or(#facade::RegisterCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let required = offset.checked_add(#definition::SPEC.width()).ok_or(#facade::RegisterCodecError::BufferTooShort { required: usize::MAX, actual: self.bytes.len() })?; let actual = self.bytes.len(); let bytes = self.bytes.get_mut(offset..required).ok_or(#facade::RegisterCodecError::BufferTooShort { required, actual })?; #view_mut::new(bytes) });
    (getter, mutable)
}

fn suffixed_path(path: &Path, suffix: &str) -> Path {
    let mut output = path.clone();
    if let Some(last) = output.segments.last_mut() {
        last.ident = format_ident!("{}{}", last.ident, suffix);
    }
    output
}
