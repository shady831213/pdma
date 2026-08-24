use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::ScalarType;

pub(super) fn ty(scalar: ScalarType) -> TokenStream {
    match scalar {
        ScalarType::U8 => quote!(u8),
        ScalarType::U16 => quote!(u16),
        ScalarType::U32 => quote!(u32),
        ScalarType::U64 => quote!(u64),
    }
}

pub(super) fn kind(scalar: ScalarType) -> syn::Ident {
    format_ident!(
        "{}",
        match scalar {
            ScalarType::U8 => "U8",
            ScalarType::U16 => "U16",
            ScalarType::U32 => "U32",
            ScalarType::U64 => "U64",
        }
    )
}

pub(super) const fn for_bits(width: usize) -> ScalarType {
    if width <= 8 {
        ScalarType::U8
    } else if width <= 16 {
        ScalarType::U16
    } else if width <= 32 {
        ScalarType::U32
    } else {
        ScalarType::U64
    }
}

pub(super) fn from_u64(scalar: ScalarType, value: TokenStream) -> TokenStream {
    match scalar {
        ScalarType::U8 => {
            quote!({ let bytes = (#value).to_le_bytes(); u8::from_le_bytes([bytes[0]]) })
        }
        ScalarType::U16 => {
            quote!({ let bytes = (#value).to_le_bytes(); u16::from_le_bytes([bytes[0], bytes[1]]) })
        }
        ScalarType::U32 => {
            quote!({ let bytes = (#value).to_le_bytes(); u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) })
        }
        ScalarType::U64 => value,
    }
}

pub(super) fn from_usize(scalar: ScalarType, value: &TokenStream) -> TokenStream {
    match scalar {
        ScalarType::U8 => {
            quote!({ let bytes = (#value).to_le_bytes(); u8::from_le_bytes([bytes[0]]) })
        }
        ScalarType::U16 => {
            quote!({ let bytes = (#value).to_le_bytes(); u16::from_le_bytes([bytes[0], bytes[1]]) })
        }
        ScalarType::U32 => {
            quote!({ let bytes = (#value).to_le_bytes(); u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) })
        }
        ScalarType::U64 => quote!({
            let bytes = (#value).to_le_bytes();
            let mut output = [0_u8; 8];
            let mut index = 0_usize;
            while index < bytes.len() && index < output.len() {
                output[index] = bytes[index];
                index += 1;
            }
            u64::from_le_bytes(output)
        }),
    }
}

pub(super) fn record_size_path(path: &syn::Path) -> syn::Path {
    let mut output = path.clone();
    if let Some(last) = output.segments.last_mut() {
        if last.ident == "record_size" {
            last.ident = format_ident!("RECORD_SIZE");
        }
    }
    output
}

pub(super) fn type_name(record: &syn::Ident, field: &syn::Ident, suffix: &str) -> syn::Ident {
    let field = pascal(&field.to_string());
    format_ident!("{record}{field}{suffix}")
}

pub(super) fn suffixed_path(path: &syn::Path, field: &syn::Ident) -> syn::Path {
    let mut output = path.clone();
    if let Some(last) = output.segments.last_mut() {
        last.ident = format_ident!("{}{}", last.ident, pascal(&field.to_string()));
    }
    output
}

fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}
