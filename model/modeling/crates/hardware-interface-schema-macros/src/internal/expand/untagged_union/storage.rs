use crate::internal::syntax::UntaggedUnionDecl;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn expand(decl: &UntaggedUnionDecl, facade: &TokenStream) -> TokenStream {
    let name = &decl.name;
    let visibility = &decl.visibility;
    let raw = format_ident!("{name}Raw");
    let case = format_ident!("{name}Case");
    let views = [(format_ident!("{name}View"), false), (format_ident!("{name}ViewMut"), true)].map(|(view, mutable)| {
        let (mutability, getter, setter) = if mutable {
            (quote!(mut), quote!(get_mut), quote! {
                #[doc = "Atomically replaces the union storage."]
                pub fn set_values(&mut self, value: &#name) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                    value.encode_variant(self.bytes)
                }
            })
        } else { (quote!(), quote!(get), quote!()) };
        quote! {
            #[doc = "Extent-checked view over untagged union storage."]
            #[derive(Debug)]
            #visibility struct #view<'a> { bytes: &'a #mutability [u8] }
            impl<'a> #view<'a> {
                #[doc = "Checks the union extent and borrows its storage."]
                pub fn new(bytes: &'a #mutability [u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                    let actual = bytes.len();
                    let bytes = bytes.#getter(..#name::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #name::RECORD_SIZE, actual })?;
                    ::core::result::Result::Ok(Self { bytes })
                }
                #[doc = "Decodes the explicit caller-selected interpretation."]
                pub fn values(&self, case: #case) -> ::core::result::Result<#name, #facade::SchemaCodecError> {
                    #name::decode_variant(self.bytes, case)
                }
                #setter
            }
        }
    });
    quote! {
        #[doc = "Owned unvalidated bytes of the untagged union."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        #visibility struct #raw([u8; #name::RECORD_SIZE]);
        impl #raw {
            #[doc = "All-zero raw storage."]
            pub const ZERO: Self = Self([0_u8; #name::RECORD_SIZE]);
            #[doc = "Owns exact-size unvalidated bytes."]
            pub const fn new(bytes: [u8; #name::RECORD_SIZE]) -> Self { Self(bytes) }
            #[doc = "Borrows exact-size bytes."]
            pub const fn as_bytes(&self) -> &[u8; #name::RECORD_SIZE] { &self.0 }
            #[doc = "Returns exact-size bytes."]
            pub const fn into_bytes(self) -> [u8; #name::RECORD_SIZE] { self.0 }
        }
        impl ::core::default::Default for #raw { fn default() -> Self { Self::ZERO } }
        impl ::core::convert::AsRef<[u8]> for #raw { fn as_ref(&self) -> &[u8] { &self.0 } }
        impl ::core::convert::AsMut<[u8]> for #raw { fn as_mut(&mut self) -> &mut [u8] { &mut self.0 } }
        #(#views)*
    }
}
