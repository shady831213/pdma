use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use crate::internal::{
    expand::{bits, scalar},
    syntax::BitFieldDecl,
    validate,
};

pub(super) fn expand(
    field: &BitFieldDecl,
    register_width: usize,
    marker: &syn::Ident,
    facade: &TokenStream,
) -> TokenStream {
    let method = &field.name;
    let rust_docs = field.documentation.rust_docs();
    let semantic_type = bits::field_type(marker, field);
    let with_method = format_ident!("with_{}", method);
    let lsb = field.start.0;
    let bit_width = bits::width(field);
    let maximum = validate::bits::bit_mask(bit_width);
    let field_mask = maximum << lsb;
    let register_scalar = format_ident!("u{}", register_width * 8);
    let narrowed = super::scalar_from_u64(&register_scalar, register_width, &quote!(combined));
    if let Some(decl) = &field.enumeration {
        let raw = scalar::from_u64(decl.storage, quote!((u64::from(self.0) >> #lsb) & #maximum));
        let observed = widen_bytes(&quote!(value.raw()), decl.storage.width(), register_width);
        let mask = super::scalar_from_u64(&register_scalar, register_width, &quote!(#field_mask));
        return quote! {
            #rust_docs
            #[doc = "Decodes the named enum field."]
            pub fn #method(self) -> ::core::result::Result<#semantic_type, #facade::RegisterCodecError> {
                #semantic_type::from_raw(#raw).map_err(#facade::RegisterCodecError::from_schema_error)
            }
            #rust_docs
            #[doc = "Replaces an enum field while preserving every other bit."]
            pub const fn #with_method(self, value: #semantic_type) -> Self {
                let observed = #observed;
                Self((self.0 & !#mask) | (observed << #lsb))
            }
        };
    }
    let identity = format_ident!(
        "{}_IDENTITY",
        method.unraw().to_string().to_ascii_uppercase()
    );
    let decoded_value = bits::decode(field, &quote!(u64::from(self.0)));
    quote! {
        #rust_docs
        #[doc = "Reads the named register field from the owned value."]
        pub fn #method(self) -> #semantic_type { #decoded_value }
        #rust_docs
        #[doc = "Replaces the named field while preserving every other register bit."]
        pub fn #with_method(self, value: #semantic_type) -> ::core::result::Result<Self, #facade::RegisterCodecError> {
            let observed = u64::from(value);
            if observed > #maximum {
                return ::core::result::Result::Err(#facade::RegisterCodecError::FieldValueOverflow { location: #marker::#identity.at_bit(#lsb), bit_width: #bit_width, value: observed });
            }
            let combined = (u64::from(self.0) & !#field_mask) | (observed << #lsb);
            ::core::result::Result::Ok(Self(#narrowed))
        }
    }
}

fn widen_bytes(value: &TokenStream, input_width: usize, output_width: usize) -> TokenStream {
    if input_width == output_width {
        return value.clone();
    }
    let scalar = format_ident!("u{}", output_width * 8);
    let bytes = (0..output_width).map(|index| {
        if index < input_width {
            let index = syn::Index::from(index);
            quote!(bytes[#index])
        } else {
            quote!(0)
        }
    });
    quote!({ let bytes = (#value).to_le_bytes(); #scalar::from_le_bytes([#(#bytes),*]) })
}
