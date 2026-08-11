//! Function-like schema macros for `hardware-interface-schema`.

#![forbid(unsafe_code)]
#![allow(
    clippy::redundant_pub_crate,
    reason = "proc-macro crates cannot export helper modules while unreachable_pub requires crate visibility"
)]

pub(crate) mod crate_path;
pub(crate) mod expand;
pub(crate) mod syntax;
pub(crate) mod validate;

use proc_macro::TokenStream;

/// Declares one reusable typed scalar field definition.
#[proc_macro]
pub fn field_schema(input: TokenStream) -> TokenStream {
    expand::field::run(input.into()).into()
}

/// Declares a fixed byte-record schema and checked scalar views.
#[proc_macro]
pub fn record_schema(input: TokenStream) -> TokenStream {
    expand::record::run(input.into()).into()
}

/// Declares a named set of typed register specifications.
#[proc_macro]
pub fn register_schema(input: TokenStream) -> TokenStream {
    expand::register::run(input.into()).into()
}

/// Declares one reusable register bit-field definition.
#[proc_macro]
pub fn register_field_schema(input: TokenStream) -> TokenStream {
    expand::register_field::run(input.into()).into()
}

/// Declares a fixed and repeated-bank register map with exact-start lookup.
#[proc_macro]
pub fn register_map(input: TokenStream) -> TokenStream {
    expand::map::run(input.into()).into()
}
