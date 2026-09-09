use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub(super) rust_docs: &'a TokenStream,
    pub(super) visibility: &'a syn::Visibility,
    pub(super) values: &'a syn::Ident,
    pub(super) raw: &'a syn::Ident,
    pub(super) view: &'a syn::Ident,
    pub(super) scalar: &'a syn::Ident,
    pub(super) width: usize,
    pub(super) reset_scalar: &'a TokenStream,
    pub(super) reset_bytes: &'a TokenStream,
    pub(super) endian: &'a syn::Ident,
    pub(super) value_methods: &'a [TokenStream],
    pub(super) facade: &'a TokenStream,
}

pub(super) fn expand(context: Context<'_>) -> TokenStream {
    let Context {
        rust_docs,
        visibility,
        values,
        raw,
        view,
        scalar,
        width,
        reset_scalar,
        reset_bytes,
        endian,
        value_methods,
        facade,
    } = context;
    quote! {
        #rust_docs
        #[doc = "Owned full-width semantic value for the generated register."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #values(#scalar);
        impl #values {
            #[doc = "All-zero full-width register value."]
            pub const ZERO: Self = Self(0);
            #[doc = "Declared register reset value."]
            pub const RESET: Self = Self(#reset_scalar);
            #[doc = "Creates one full-width register value."]
            pub const fn new(raw: #scalar) -> Self { Self(raw) }
            #[doc = "Returns the complete register scalar."]
            pub const fn raw(self) -> #scalar { self.0 }
            #(#value_methods)*
        }
        impl ::core::default::Default for #values {
            fn default() -> Self { Self::RESET }
        }
        impl ::core::convert::From<#scalar> for #values {
            fn from(raw: #scalar) -> Self { Self::new(raw) }
        }
        impl ::core::convert::From<#values> for #scalar {
            fn from(value: #values) -> Self { value.raw() }
        }
        #rust_docs
        #[doc = "Owned typed raw bytes for the generated register."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #raw([u8; #width]);
        impl #raw {
            #[doc = "All-zero unvalidated register bytes."]
            pub const ZERO: Self = Self([0_u8; #width]);
            #[doc = "Creates one raw register owner."]
            pub const fn new(bytes: [u8; #width]) -> Self { Self(bytes) }
            #[doc = "Borrows the exact register bytes."]
            pub const fn as_bytes(&self) -> &[u8; #width] { &self.0 }
            #[doc = "Returns the exact register bytes."]
            pub const fn into_bytes(self) -> [u8; #width] { self.0 }
        }
        impl ::core::default::Default for #raw {
            fn default() -> Self { Self(#reset_bytes) }
        }
        impl ::core::convert::From<[u8; #width]> for #raw {
            fn from(bytes: [u8; #width]) -> Self { Self::new(bytes) }
        }
        impl ::core::convert::From<#raw> for [u8; #width] {
            fn from(raw: #raw) -> Self { raw.into_bytes() }
        }
        impl #facade::Decode for #raw {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #facade::read_scalar(&self.0, #facade::Endian::#endian).map(#values::new)
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                let mut bytes = [0_u8; #width];
                #facade::write_scalar(&mut bytes, #facade::Endian::#endian, self.0)?;
                ::core::result::Result::Ok(#raw::new(bytes))
            }
        }
        impl<'a> #facade::Decode for #view<'a> {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #facade::read_scalar(self.bytes, #facade::Endian::#endian).map(#values::new)
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
