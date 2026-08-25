use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub(super) visibility: &'a syn::Visibility,
    pub(super) values: &'a syn::Ident,
    pub(super) raw: &'a syn::Ident,
    pub(super) tag: &'a TokenStream,
    pub(super) first_case: &'a syn::Ident,
    pub(super) extent: usize,
    pub(super) facade: &'a TokenStream,
}

pub(super) fn expand(context: Context<'_>) -> TokenStream {
    let Context {
        visibility,
        values,
        raw,
        tag,
        first_case,
        extent,
        facade,
    } = context;
    quote! {
        #[doc = "Typed raw tag and storage for the generated union."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #raw {
            tag: #tag,
            bytes: [u8; #extent],
        }
        impl #raw {
            #[doc = "Creates one typed raw union value."]
            pub const fn new(tag: #tag, bytes: [u8; #extent]) -> Self { Self { tag, bytes } }
            #[doc = "Returns the raw union tag."]
            pub const fn tag(self) -> #tag { self.tag }
            #[doc = "Borrows the raw union storage."]
            pub const fn as_bytes(&self) -> &[u8; #extent] { &self.bytes }
            #[doc = "Returns the raw union tag and storage."]
            pub const fn into_parts(self) -> (#tag, [u8; #extent]) { (self.tag, self.bytes) }
        }
        impl ::core::default::Default for #raw {
            fn default() -> Self { Self::new(#tag::#first_case, [0_u8; #extent]) }
        }
        impl #facade::Decode for #raw {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                #values::decode(&self.bytes, 0, self.tag)
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                let mut bytes = [0_u8; #extent];
                self.encode_into(&mut bytes, 0)?;
                ::core::result::Result::Ok(#raw::new(self.tag(), bytes))
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
