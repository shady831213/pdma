use proc_macro2::TokenStream;
use syn::parse2;

use crate::internal::syntax::ModernRecordDecl;

mod modern;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<ModernRecordDecl>(input)
        .and_then(|decl| modern::expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}
