#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod internal;

use proc_macro::TokenStream;

trait Expansion {
    fn bits(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn untagged_union(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn record(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn register(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
    fn map(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream;
}

/// Declares one reusable typed bit-field schema.
///
/// Invoke through `hw_model_schema::bits_schema!`. Fields describe bit
/// ranges in one scalar storage word; the generated value checks field widths.
/// See the facade's crate documentation for declaration syntax and examples.
#[proc_macro]
pub fn bits_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::bits(input.into()).into()
}

/// Declares an inferred typed record schema and checked views.
///
/// Invoke through `hw_model_schema::record_schema!`. Generates layout
/// metadata, borrowed byte views and a codec for scalar and composed fields.
/// A `tagged_union` field selects its variant using the enclosing record's tag.
#[proc_macro]
pub fn record_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::record(input.into()).into()
}

/// Declares one reusable typed register definition.
///
/// Invoke through `hw_model_schema::register_schema!`. Width, byte
/// order, access permissions, reset value and writable mask describe the register;
/// placement in an address space belongs to `register_map!`.
#[proc_macro]
pub fn register_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::register(input.into()).into()
}

/// Declares one ordered composable register map with exact-start lookup.
///
/// Invoke through `hw_model_schema::register_map!`. Places registers,
/// nested maps and repeated banks; generated layouts supply offsets and extents.
/// Hardware effects remain in the caller's register-access hooks.
#[proc_macro]
pub fn register_map(input: TokenStream) -> TokenStream {
    <() as Expansion>::map(input.into()).into()
}

/// Declares an untagged union of schema layouts without choosing a variant.
///
/// Invoke through `hw_model_schema::union_schema!`. Generates layout
/// and explicit per-variant codec helpers, but no whole-union selection rule.
/// Applications may implement the facade's `SchemaCodec` when their protocol
/// supplies that rule. A variant used for encoding or decoding must itself
/// implement `SchemaCodec`; layout alone is sufficient for layout composition.
#[proc_macro]
pub fn union_schema(input: TokenStream) -> TokenStream {
    <() as Expansion>::untagged_union(input.into()).into()
}
