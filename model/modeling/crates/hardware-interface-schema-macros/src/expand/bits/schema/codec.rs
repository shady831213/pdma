use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn expand(
    values: &syn::Ident,
    raw_name: &syn::Ident,
    raw_type: &syn::Ident,
    declared_mask: u64,
    narrowed_raw: &TokenStream,
    facade: &TokenStream,
) -> TokenStream {
    quote! {
        impl #facade::Decode for #raw_name {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                let observed = u64::from(self.0);
                let reserved = observed & !#declared_mask;
                if reserved != 0 {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved { offset: 0, observed: reserved });
                }
                ::core::result::Result::Ok(#values::from_raw(observed))
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw_name;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                let raw: u64 = self.raw(0).map_err(#facade::BitsValueError::into_schema_error)?;
                let raw: #raw_type = #narrowed_raw;
                ::core::result::Result::Ok(#raw_name::new(raw))
            }
        }
        impl ::core::convert::TryFrom<#raw_name> for #values {
            type Error = #facade::SchemaCodecError;
            fn try_from(raw: #raw_name) -> ::core::result::Result<Self, Self::Error> {
                <#raw_name as #facade::Decode>::decode(raw)
            }
        }
        impl ::core::convert::TryFrom<#values> for #raw_name {
            type Error = #facade::SchemaCodecError;
            fn try_from(value: #values) -> ::core::result::Result<Self, Self::Error> {
                <#values as #facade::Encode>::encode(value)
            }
        }
    }
}
