//! Generic C11 fragment exporter integration tests.

#![cfg(feature = "export-c-header")]

#[allow(missing_docs)]
pub mod support;

mod declarations;
mod descriptions;
mod enum_bit_union;
mod enumeration;
mod generated_enumeration;
mod geometry;
mod ordered_descriptions;
mod record_tests;
mod register_map_tests;
mod sink_errors;
mod untagged;
