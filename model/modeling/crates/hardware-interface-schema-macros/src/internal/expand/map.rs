use proc_macro2::TokenStream;
use syn::parse2;

use crate::internal::syntax::OrderedMapDecl;

mod ordered;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<OrderedMapDecl>(input)
        .and_then(|decl| ordered::expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}
