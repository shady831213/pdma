#![doc = include_str!("../README.md")]

/// Scalar encoding and decoding functions plus their typed errors.
pub mod codec;
/// Byte-order vocabulary for scalar codecs and record fields.
pub mod endian;
/// Format-neutral artifact and exporter extension boundary.
pub mod export;
/// Fixed-record field and validation vocabulary.
pub mod record;
/// Register metadata vocabulary.
pub mod register;
/// Register-map vocabulary and exact-start lookup surface.
pub mod register_map;
/// Supported scalar kinds and their sealed codec trait.
pub mod scalar;

pub use codec::{SchemaCodecError, read_scalar, write_scalar};
pub use endian::Endian;
pub use hardware_interface_schema_macros::{record_schema, register_map, register_schema};
pub use record::{FieldSpec, RecordSpec, RecordValidationError};
pub use scalar::{Scalar, ScalarKind};
