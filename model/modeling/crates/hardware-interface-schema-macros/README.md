# Hardware interface schema macros

Procedural macros implementing `hardware-interface-schema` records, register maps,
and register dispatch. Applications should use the facade crate, which re-exports
these macros and supplies the types their generated code uses. Renamed facade
dependencies are supported.

This crate has no device-model or runtime dependency. It is unpublished; its package
name and release contents remain subject to review.

Licensed under MIT OR Apache-2.0, at your option. See LICENSE-MIT and LICENSE-APACHE.
