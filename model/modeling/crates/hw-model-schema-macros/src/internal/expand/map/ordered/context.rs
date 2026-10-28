use proc_macro2::TokenStream;

pub(super) struct Geometry {
    pub(super) rust_docs: TokenStream,
    pub(super) symbol: syn::Ident,
    pub(super) definition: syn::Path,
    pub(super) placement: syn::Ident,
    pub(super) count: usize,
    pub(super) bank: bool,
}

pub(super) struct ExpansionContext<'a> {
    pub(super) facade: &'a TokenStream,
    pub(super) marker: &'a syn::Ident,
    pub(super) values_name: &'a syn::Ident,
    pub(super) view_name: &'a syn::Ident,
    pub(super) view_mut_name: &'a syn::Ident,
    pub(super) extent: &'a TokenStream,
}
