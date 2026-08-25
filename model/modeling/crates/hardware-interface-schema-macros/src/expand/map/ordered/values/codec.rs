use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn expand(
    visibility: &syn::Visibility,
    raw: &syn::Ident,
    values: &syn::Ident,
    view: &syn::Ident,
    extent: &TokenStream,
    facade: &TokenStream,
) -> TokenStream {
    quote! {
        #[doc = "Owned typed raw bytes for the generated register map."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #raw([u8; #extent]);
        impl #raw {
            #[doc = "All-zero unvalidated map bytes."]
            pub const ZERO: Self = Self([0_u8; #extent]);
            #[doc = "Creates one raw map owner."]
            pub const fn new(bytes: [u8; #extent]) -> Self { Self(bytes) }
            #[doc = "Borrows the exact map bytes."]
            pub const fn as_bytes(&self) -> &[u8; #extent] { &self.0 }
            #[doc = "Returns the exact map bytes."]
            pub const fn into_bytes(self) -> [u8; #extent] { self.0 }
        }
        impl ::core::convert::From<[u8; #extent]> for #raw {
            fn from(bytes: [u8; #extent]) -> Self { Self::new(bytes) }
        }
        impl ::core::convert::From<#raw> for [u8; #extent] {
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
                self.to_bytes().map(#raw::new)
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
