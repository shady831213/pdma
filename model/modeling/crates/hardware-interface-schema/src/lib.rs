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
pub use hardware_interface_schema_macros::{
    field_schema, record_schema, register_field_schema, register_map, register_schema,
};
pub use record::{
    FieldDefinition, FieldSpec, RecordGraphError, RecordPlacement, RecordSpec,
    RecordValidationError, ResolvedMember, ResolvedMemberKind, ResolvedRecord,
};
pub use register::{
    Access, RegisterDefinition, RegisterDefinitionError, RegisterFieldDefinition,
    RegisterFieldSpec, RegisterSpec, RegisterValidationError, RegisterValueError,
    RegisterValueView, RegisterValueViewMut, ResetKind,
};
pub use register_map::{
    RegisterLocation, RegisterMapError, RegisterMapSpec, RegisterMapValidationError,
    RepeatedRegisterBank, ResolvedRegister, ResolvedRegisterBank, ResolvedRegisterMap,
};
pub use scalar::{Scalar, ScalarKind};

/// Typed contract implemented by markers generated with [`field_schema!`].
pub trait FieldSchema {
    /// Rust scalar encoded by the definition.
    type Scalar: Scalar;

    /// Reusable scalar definition.
    const DEFINITION: FieldDefinition;

    /// Reads a scalar from the start of a checked byte slice.
    fn read(bytes: &[u8]) -> Result<Self::Scalar, SchemaCodecError>;

    /// Writes a scalar at the start of a checked byte slice.
    fn write(bytes: &mut [u8], value: Self::Scalar) -> Result<(), SchemaCodecError>;
}

/// Failure while inserting a value through a generated register-field marker.
pub type RegisterCodecError = RegisterValueError;
