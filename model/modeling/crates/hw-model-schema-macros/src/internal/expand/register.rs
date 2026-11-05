use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse2;

use crate::internal::syntax::RegisterDefinitionDecl;

mod definition;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<RegisterDefinitionDecl>(input)
        .and_then(|decl| {
            let body = definition::expand(&decl)?;
            let facade = crate::internal::crate_path::facade()?;
            let ast = super::ast::register(&decl, &facade);
            Ok(quote!(#body #ast))
        })
        .unwrap_or_else(syn::Error::into_compile_error)
}

pub(in crate::internal) fn variant(name: &syn::Ident) -> syn::Ident {
    let converted = name
        .unraw()
        .to_string()
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<String>();
    syn::parse_str::<syn::Ident>(&converted).unwrap_or_else(|_| name.unraw())
}
