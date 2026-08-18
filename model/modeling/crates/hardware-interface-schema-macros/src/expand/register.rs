use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse2;

use crate::internal::{
    crate_path,
    syntax::{AccessValue, RegisterDecl, RegisterInput, RegistersDecl},
    validate,
};

mod definition;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<RegisterInput>(input)
        .and_then(|decl| match decl {
            RegisterInput::Legacy(decl) => expand(&decl),
            RegisterInput::Definition(decl) => definition::expand(&decl),
        })
        .unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &RegistersDecl) -> syn::Result<TokenStream> {
    validate::register::check(&decl.registers, true)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let generated_id = format_ident!("{}RegisterId", marker);
    let id_name = decl
        .external_id
        .as_ref()
        .map_or_else(|| quote!(#generated_id), |path| quote!(#path));
    let raw_variants: Vec<_> = decl
        .registers
        .iter()
        .map(|register| variant(&register.symbol.to_string()))
        .collect();
    let variants: Vec<_> = raw_variants
        .iter()
        .map(|variant| quote!(#[doc = "Stable generated register ID."] #variant))
        .collect();
    let specs: Vec<_> = decl
        .registers
        .iter()
        .zip(&raw_variants)
        .map(|(register, variant)| {
            let id = quote!(#id_name::#variant);
            spec(register, &id, &id_name, &facade)
        })
        .collect();
    let constants: Vec<_> = decl
        .registers
        .iter()
        .zip(&raw_variants)
        .map(|(register, variant)| {
            let constant = format_ident!("{}", register.symbol.to_string().to_uppercase());
            let id = quote!(#id_name::#variant);
            let value = spec(register, &id, &id_name, &facade);
            quote!(#[doc = "Named register specification."] pub const #constant: #facade::register::RegisterSpec<#id_name> = #value;)
        })
        .collect();
    let id_declaration = decl.external_id.is_none().then(|| quote! {
        #[doc = "Stable identities for the generated register set."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #generated_id { #(#variants),* }
    });
    let register_lookup = decl.external_id.is_none().then(|| quote! {
        #[doc = "Returns the specification for a typed register ID."]
        pub const fn register(id: #id_name) -> #facade::register::RegisterSpec<#id_name> { match id { #(#id_name::#raw_variants => Self::SPEC[#id_name::#raw_variants as usize]),* } }
    });
    Ok(quote! {
        #id_declaration
        #[doc = "Marker and metadata namespace for the generated register set."]
        #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #(#constants)*
            #[doc = "All generated register specifications in declaration order."]
            pub const SPEC: &'static [#facade::register::RegisterSpec<#id_name>] = &[#(#specs),*];
            #register_lookup
        }
    })
}

pub(in crate::internal) fn spec(
    register: &RegisterDecl,
    id: &TokenStream,
    id_name: &TokenStream,
    facade: &TokenStream,
) -> TokenStream {
    let name = &register.name;
    let numeric_id = register.id.0;
    let offset = register.offset.0;
    let width = register.width.0;
    let reset_kind = &register.reset.kind;
    let reset_value = register.reset.value.0;
    let mask = register.mask.0;
    let access = match register.access {
        AccessValue::ReadOnly => format_ident!("ReadOnly"),
        AccessValue::ReadWrite => format_ident!("ReadWrite"),
        AccessValue::WriteOnly => format_ident!("WriteOnly"),
    };
    quote!(#facade::register::RegisterSpec::<#id_name>::new(#id, #name, #offset, #width).with_numeric_id(#numeric_id).with_access(#facade::register::Access::#access).with_reset(#facade::register::ResetKind::#reset_kind, #reset_value).with_writable_mask(#mask))
}

pub(in crate::internal) fn variant(name: &str) -> syn::Ident {
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
