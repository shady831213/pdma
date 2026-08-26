use proc_macro2::TokenStream;
use quote::format_ident;
use syn::parse2;

use crate::internal::syntax::RegisterDefinitionDecl;

mod definition;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<RegisterDefinitionDecl>(input)
        .and_then(|decl| definition::expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
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
