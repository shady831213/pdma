use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn expand(
    visibility: &syn::Visibility,
    values: &syn::Ident,
    raw: &syn::Ident,
    view: &syn::Ident,
    size: usize,
    facade: &TokenStream,
) -> TokenStream {
    quote! {
        #[doc = "Owned typed raw bytes for the generated record."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #raw([u8; #size]);
        impl #raw {
            #[doc = "All-zero unvalidated raw bytes."]
            pub const ZERO: Self = Self([0_u8; #size]);
            #[doc = "Creates one raw owner from exact-size bytes."]
            pub const fn new(bytes: [u8; #size]) -> Self { Self(bytes) }
            #[doc = "Borrows the exact raw bytes."]
            pub const fn as_bytes(&self) -> &[u8; #size] { &self.0 }
            #[doc = "Returns the exact raw bytes."]
            pub const fn into_bytes(self) -> [u8; #size] { self.0 }
        }
        impl ::core::default::Default for #raw {
            fn default() -> Self { Self::ZERO }
        }
        impl ::core::convert::From<[u8; #size]> for #raw {
            fn from(bytes: [u8; #size]) -> Self { Self::new(bytes) }
        }
        impl ::core::convert::From<#raw> for [u8; #size] {
            fn from(raw: #raw) -> Self { raw.into_bytes() }
        }
        impl #facade::Decode for #raw {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #values::decode_from(&self.0)
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                self.encoded().map(#raw::new)
            }
        }
        impl<'a> #facade::Decode for #view<'a> {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #values::decode_from(self.bytes)
            }
        }
        impl ::core::convert::TryFrom<#raw> for #values {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw) -> ::core::result::Result<Self, Self::Error> {
                <#raw as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#values> for #raw {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #values) -> ::core::result::Result<Self, Self::Error> {
                <#values as #facade::Encode>::encode(value)
            }
        }
    }
}
