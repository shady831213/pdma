use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use crate::internal::syntax::TaggedUnionFieldDecl;

pub(super) fn expand(
    owner: &syn::Ident,
    fields: &[TaggedUnionFieldDecl],
    facade: &TokenStream,
) -> TokenStream {
    let identities = fields.iter().map(|field| {
        let name = format_ident!(
            "{}_IDENTITY",
            field.name.unraw().to_string().to_ascii_uppercase()
        );
        let field_name = field.name.to_string();
        quote! {
            #[doc = "Stable generated identity for the union-case field."]
            pub const #name: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(stringify!(#owner), #field_name);
        }
    });
    quote! {
        impl #owner {
            #(#identities)*
        }
    }
}
