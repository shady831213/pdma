//! Function-like schema macros for `hardware-interface-schema`.

#![forbid(unsafe_code)]

mod internal;

use proc_macro::TokenStream;

trait Expansion {
    fn bits(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn record(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn register(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn map(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
}

/// Declares one reusable typed bit-field schema.
#[proc_macro]
pub fn bits_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::bits(input.into()).into()
}

/// Declares an inferred typed record schema and checked views.
#[proc_macro]
pub fn record_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::record(input.into()).into()
}

/// Declares one reusable typed register definition.
#[proc_macro]
pub fn register_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::register(input.into()).into()
}

/// Declares one ordered composable register map with exact-start lookup.
#[proc_macro]
pub fn register_map(input: TokenStream) -> TokenStream {
    <() as Expansion>::map(input.into()).into()
}
