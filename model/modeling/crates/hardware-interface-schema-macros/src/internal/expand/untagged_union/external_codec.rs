use crate::internal::syntax::UntaggedUnionDecl;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn expand(decl: &UntaggedUnionDecl, facade: &TokenStream) -> TokenStream {
    let name = &decl.name;
    let raw = format_ident!("{name}Raw");
    let view = format_ident!("{name}View");
    let view_mut = format_ident!("{name}ViewMut");
    quote! {
        impl #name {
            #[doc = "Encodes exact-size bytes through the application-provided codec."]
            pub fn to_bytes(self) -> ::core::result::Result<[u8; Self::RECORD_SIZE], #facade::SchemaCodecError> {
                let mut bytes = [0_u8; Self::RECORD_SIZE];
                self.encode_into(&mut bytes)?;
                ::core::result::Result::Ok(bytes)
            }
        }
        impl #facade::Encode for #name {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                self.to_bytes().map(#raw::new)
            }
        }
        impl #facade::Decode for #raw {
            type Decoded = #name;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #name::from_bytes(self.as_bytes())
            }
        }
        impl ::core::convert::TryFrom<#raw> for #name {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw) -> ::core::result::Result<Self, Self::Error> {
                <#raw as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#name> for #raw {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #name) -> ::core::result::Result<Self, Self::Error> {
                <#name as #facade::Encode>::encode(value)
            }
        }
        impl #view<'_> {
            #[doc = "Decodes through the application-provided codec."]
            pub fn values(&self) -> ::core::result::Result<#name, #facade::SchemaCodecError> {
                #name::from_bytes(self.bytes)
            }
        }
        impl #view_mut<'_> {
            #[doc = "Decodes through the application-provided codec."]
            pub fn values(&self) -> ::core::result::Result<#name, #facade::SchemaCodecError> {
                #name::from_bytes(self.bytes)
            }
            #[doc = "Replaces storage through the application-provided codec."]
            pub fn set_values(&mut self, value: &#name) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                value.encode_into(self.bytes)
            }
        }
        impl #facade::Decode for #view<'_> {
            type Decoded = #name;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #name::from_bytes(self.bytes)
            }
        }
        impl #facade::RecordLayout for #name {
            type Value = Self;
            type View<'a> = #view<'a>;
            type ViewMut<'a> = #view_mut<'a>;
            const NAME: &'static str = stringify!(#name);
            const DOCUMENTATION: #facade::Documentation = Self::SPEC.documentation();
            const ALIGNMENT: usize = Self::RECORD_ALIGNMENT;
            const EXTENT: usize = Self::RECORD_SIZE;
            const ZERO: Self = Self::ZERO;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                Self::from_bytes(bytes)
            }
            fn encode(bytes: &mut [u8], value: &Self) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                value.encode_into(bytes)
            }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> {
                #view::new(bytes)
            }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> {
                #view_mut::new(bytes)
            }
        }
    }
}
