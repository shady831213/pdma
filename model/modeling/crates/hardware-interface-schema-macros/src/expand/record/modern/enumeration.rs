use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::{syntax::EnumDecl, validate::record::modern::enum_variants};

use super::scalar;

pub(super) fn expand(
    record: &syn::Ident,
    field: &syn::Ident,
    decl: &EnumDecl,
    visibility: &syn::Visibility,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let name = scalar::type_name(record, field, "");
    let raw_type = scalar::ty(decl.storage);
    let variants = enum_variants(decl)?;
    let declarations = variants
        .iter()
        .map(|(variant, _)| quote!(#[doc = "Generated enum variant."] #variant));
    let from_arms = variants
        .iter()
        .map(|(variant, value)| quote!(#value => ::core::result::Result::Ok(Self::#variant),));
    let raw_arms = variants.iter().map(|(variant, value)| {
        let value = scalar::from_u64(decl.storage, quote!(#value));
        quote!(Self::#variant => #value,)
    });
    let Some((first, _)) = variants.first() else {
        return Err(syn::Error::new(field.span(), "enum requires a variant"));
    };
    Ok(quote! {
        #[doc = "Generated typed record enum."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#declarations),* }
        impl #name {
            #[doc = "First declared value used by generated zero records."]
            pub const ZERO: Self = Self::#first;
            #[doc = "Parses one raw discriminant."]
            pub fn from_raw(raw: #raw_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                match u64::from(raw) {
                    #(#from_arms)*
                    observed => ::core::result::Result::Err(#facade::SchemaCodecError::UnknownTag { observed }),
                }
            }
            #[doc = "Returns the assigned raw discriminant."]
            pub const fn raw(self) -> #raw_type {
                match self { #(#raw_arms)* }
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
    })
}
