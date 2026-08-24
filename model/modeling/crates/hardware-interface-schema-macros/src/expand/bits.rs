use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;

use crate::internal::syntax::{BitFieldDecl, BitsSchemaDecl};

mod schema;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    parse2::<BitsSchemaDecl>(input)
        .and_then(|decl| schema::expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}

pub(in crate::internal) fn semantic_type(field: &BitFieldDecl) -> TokenStream {
    integer_type(width(field))
}

pub(in crate::internal) fn zero(field: &BitFieldDecl) -> TokenStream {
    if width(field) == 1 {
        quote!(false)
    } else {
        quote!(0)
    }
}

pub(in crate::internal) fn decode(field: &BitFieldDecl, observed: &TokenStream) -> TokenStream {
    let start = field.start.0;
    let width = width(field);
    let mask = crate::internal::validate::bits::bit_mask(width);
    if width == 1 {
        quote!(((#observed >> #start) & 1) != 0)
    } else {
        let value = quote!((#observed >> #start) & #mask);
        narrow(width, &value)
    }
}

pub(in crate::internal) fn encode(field: &BitFieldDecl, value: &TokenStream) -> TokenStream {
    let start = field.start.0;
    quote!(u64::from(#value) << #start)
}

pub(in crate::internal) fn decode_local(
    field: &BitFieldDecl,
    observed: &TokenStream,
) -> TokenStream {
    if width(field) == 1 {
        quote!(#observed != 0)
    } else {
        narrow(width(field), observed)
    }
}

pub(in crate::internal) fn encode_local(value: &TokenStream) -> TokenStream {
    quote!(u64::from(#value))
}

pub(in crate::internal) const fn width(field: &BitFieldDecl) -> usize {
    field.end.0 - field.start.0
}

fn integer_type(width: usize) -> TokenStream {
    if width == 1 {
        quote!(bool)
    } else if width <= 8 {
        quote!(u8)
    } else if width <= 16 {
        quote!(u16)
    } else if width <= 32 {
        quote!(u32)
    } else {
        quote!(u64)
    }
}

fn narrow(width: usize, value: &TokenStream) -> TokenStream {
    if width <= 8 {
        quote!({ let bytes = (#value).to_le_bytes(); u8::from_le_bytes([bytes[0]]) })
    } else if width <= 16 {
        quote!({ let bytes = (#value).to_le_bytes(); u16::from_le_bytes([bytes[0], bytes[1]]) })
    } else if width <= 32 {
        quote!({ let bytes = (#value).to_le_bytes(); u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) })
    } else {
        quote!(#value)
    }
}
