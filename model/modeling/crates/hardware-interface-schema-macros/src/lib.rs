//! Function-like schema macros for `hardware-interface-schema`.

#![forbid(unsafe_code)]

mod internal;

use proc_macro::TokenStream;

trait Expansion {
    fn field(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn record(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn register(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn register_field(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn map(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
}

/// Declares one reusable typed scalar field definition.
#[proc_macro]
pub fn field_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::field(input.into()).into()
}

/// Declares a fixed byte-record schema and checked scalar views.
#[proc_macro]
pub fn record_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::record(input.into()).into()
}

/// Declares a named set of typed register specifications.
#[proc_macro]
pub fn register_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::register(input.into()).into()
}

/// Declares one reusable register bit-field definition.
#[proc_macro]
pub fn register_field_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::register_field(input.into()).into()
}

/// Declares a fixed and repeated-bank register map with exact-start lookup.
#[proc_macro]
pub fn register_map(input: TokenStream) -> TokenStream {
    <() as Expansion>::map(input.into()).into()
}
