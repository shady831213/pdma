use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;

use crate::internal::syntax::OrderedMapDecl;

mod instance;
mod ordered;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<OrderedMapDecl>(input)
        .and_then(|decl| {
            let body = ordered::expand(&decl)?;
            let facade = crate::internal::crate_path::facade()?;
            let ast = super::ast::register_map(&decl, &facade);
            Ok(quote!(#body #ast))
        })
        .unwrap_or_else(syn::Error::into_compile_error)
}
