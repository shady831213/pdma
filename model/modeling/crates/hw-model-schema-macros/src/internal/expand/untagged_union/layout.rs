use crate::internal::syntax::UntaggedUnionDecl;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn expand(decl: &UntaggedUnionDecl, facade: &TokenStream) -> TokenStream {
    let name = &decl.name;
    let view = format_ident!("{name}View");
    let view_mut = format_ident!("{name}ViewMut");
    let graph = crate::internal::expand::graph::union(decl, facade);
    quote! {
        #graph
        impl #facade::SchemaLayout for #name {
            fn schema_definition() -> #facade::graph::Definition {
                <Self as #facade::graph::DescribeSchema>::schema_definition()
            }
            type Value = Self;
            type View<'a> = #view<'a>;
            type ViewMut<'a> = #view_mut<'a>;
            const NAME: &'static str = stringify!(#name);
            const DOCUMENTATION: #facade::Documentation = Self::SPEC.documentation();
            const ALIGNMENT: usize = Self::RECORD_ALIGNMENT;
            const EXTENT: usize = Self::RECORD_SIZE;
            const SPEC: #facade::RecordSpec = Self::SPEC;
            const ZERO: Self = Self::ZERO;
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> {
                #view::new(bytes)
            }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> {
                #view_mut::new(bytes)
            }
        }
    }
}
