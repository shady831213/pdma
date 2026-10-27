# Hardware interface schema macros

Procedural macros implementing `hardware-interface-schema` bits, records, untagged
unions, registers and register maps. Applications should use the facade crate, which re-exports
these macros and supplies the types their generated code uses. Renamed facade
dependencies are supported.

`bits_schema!` describes fields within a storage word. `record_schema!` composes
scalar fields and nested layouts, including tagged-union fields. `union_schema!`
overlays variant layouts without selecting a variant; applications provide a
whole-union codec only when their protocol defines selection. `register_schema!`
describes one register, while `register_map!` places registers and nested banks.
The facade also supplies the declarative `register_dispatch!` access helper.

Use the facade's README and generated API documentation for syntax and executable
examples. This implementation crate is not a standalone schema API: generated code
refers to the facade's metadata, layout and codec contracts.

This crate has no device-model or runtime dependency. It is unpublished; its package
name and release contents remain subject to review.

Licensed under MIT OR Apache-2.0, at your option. See LICENSE-MIT and LICENSE-APACHE.
