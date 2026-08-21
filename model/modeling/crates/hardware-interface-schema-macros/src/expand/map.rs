use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse2;

use crate::internal::{
    crate_path,
    expand::register::{spec, variant},
    syntax::{MapDecl, MapSyntaxProbe, OrderedMapDecl},
    validate,
};

mod composable;
mod ordered;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    let expanded = parse2::<MapSyntaxProbe>(input.clone()).and_then(|probe| {
        if probe.ordered {
            parse2::<OrderedMapDecl>(input).and_then(|decl| ordered::expand(&decl))
        } else {
            parse2::<MapDecl>(input).and_then(|decl| expand(&decl))
        }
    });
    expanded.unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &MapDecl) -> syn::Result<TokenStream> {
    if !decl.referenced_fixed.is_empty()
        || decl.banks.iter().any(|bank| !bank.referenced.is_empty())
    {
        return composable::expand(decl);
    }
    validate::map::check(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let id_name = format_ident!("{}RegisterId", marker);
    let mut variants = Vec::new();
    let mut fixed_specs = Vec::new();
    let mut match_arms = Vec::new();
    for register in &decl.fixed {
        let item_variant = variant(&register.symbol.to_string());
        let id = quote!(#id_name::#item_variant);
        let id_type = quote!(#id_name);
        let item_spec = spec(register, &id, &id_type, &facade);
        variants.push(item_variant.clone());
        fixed_specs.push(item_spec.clone());
        match_arms.push(quote!(#id_name::#item_variant => #item_spec));
    }
    let mut fixed_group_specs = Vec::new();
    for group in &decl.fixed_groups {
        let schema = &group.schema;
        fixed_group_specs.push(quote!(#schema::SPEC));
        for symbol in &group.variants {
            let item_variant = variant(&symbol.to_string());
            let constant = format_ident!("{}", symbol.to_string().to_uppercase());
            variants.push(item_variant.clone());
            match_arms.push(quote!(#id_name::#item_variant => #schema::#constant));
        }
    }
    let mut bank_specs = Vec::new();
    for bank in &decl.banks {
        let bank_name = bank.name.to_string();
        let count = bank.count.0;
        let base = bank.base.0;
        let stride = bank.stride.0;
        let mut members = Vec::new();
        for register in &bank.registers {
            let item_variant = variant(&format!("{}_{}", bank.name, register.symbol));
            let id = quote!(#id_name::#item_variant);
            let id_type = quote!(#id_name);
            let item_spec = spec(register, &id, &id_type, &facade);
            variants.push(item_variant.clone());
            members.push(item_spec.clone());
            match_arms.push(quote!(#id_name::#item_variant => #item_spec));
        }
        bank_specs.push(quote!(#facade::register_map::RepeatedRegisterBank::new(#bank_name, #base, #count, #stride, &[#(#members),*])));
    }
    let size = decl.size.0;
    let documented_variants: Vec<_> = variants
        .iter()
        .map(|variant| quote!(#[doc = "Stable generated register ID."] #variant))
        .collect();
    Ok(quote! {
        #[doc = "Stable identities for the generated register map."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #visibility enum #id_name { #(#documented_variants),* }
        #[doc = "Marker and metadata namespace for the generated register map."]
        #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Complete fixed and repeated-bank map specification."]
            pub const SPEC: #facade::register_map::RegisterMapSpec<#id_name> = #facade::register_map::RegisterMapSpec::new(#size, &[#(#fixed_specs),*], &[#(#bank_specs),*]).with_register_groups(&[#(#fixed_group_specs),*]);
            #[doc = "Returns the relative specification for a typed register ID."]
            pub const fn register(id: #id_name) -> #facade::register::RegisterSpec<#id_name> { match id { #(#match_arms),* } }
            #[doc = "Locates only an exact expanded register start."]
            pub fn locate(offset: usize) -> ::core::option::Option<#facade::register_map::RegisterLocation<#id_name>> { Self::SPEC.locate(offset) }
        }
    })
}
