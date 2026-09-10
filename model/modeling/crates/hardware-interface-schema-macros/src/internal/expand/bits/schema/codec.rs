use proc_macro2::TokenStream;
use quote::quote;

pub(super) struct Context<'a> {
    pub(super) values: &'a syn::Ident,
    pub(super) raw_name: &'a syn::Ident,
    pub(super) raw_type: &'a syn::Ident,
    pub(super) declared_mask: u64,
    pub(super) narrowed_raw: &'a TokenStream,
    pub(super) facade: &'a TokenStream,
    pub(super) marker: &'a syn::Ident,
}

pub(super) fn expand(context: &Context<'_>) -> TokenStream {
    let Context {
        values,
        raw_name,
        raw_type,
        declared_mask,
        narrowed_raw,
        facade,
        marker,
    } = context;
    quote! {
        impl #facade::Decode for #raw_name {
            type Decoded = #values;
            fn decode(self) -> ::core::result::Result<Self::Decoded, #facade::SchemaCodecError> {
                let observed = u64::from(self.0);
                let reserved = observed & !#declared_mask;
                if reserved != 0 {
                    let mut first_reserved = 0_usize;
                    while reserved & (1_u64 << first_reserved) == 0 {
                        first_reserved += 1;
                    }
                    return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                        location: #marker::RESERVED_IDENTITY.at_bit(first_reserved),
                        observed: reserved,
                    });
                }
                #values::from_raw(observed)
            }
        }
        impl #facade::Encode for #values {
            type Encoded = #raw_name;
            fn encode(self) -> ::core::result::Result<Self::Encoded, #facade::SchemaCodecError> {
                let raw: u64 = self.raw().map_err(#facade::BitsValueError::into_schema_error)?;
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
