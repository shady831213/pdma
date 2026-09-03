use core::fmt;

use crate::{SchemaCoordinate, SchemaLocation};

/// Failure while validating legacy whole-register metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterValidationError {
    /// The byte width is unsupported.
    InvalidWidth {
        /// Placement name.
        name: &'static str,
        /// Supplied width.
        width: usize,
    },
    /// The writable mask exceeds the byte width.
    WritableMaskOverflow {
        /// Placement name.
        name: &'static str,
        /// Declared width.
        width: usize,
        /// Supplied mask.
        mask: u64,
    },
    /// The reset value exceeds the byte width.
    ResetValueOverflow {
        /// Placement name.
        name: &'static str,
        /// Declared width.
        width: usize,
        /// Supplied value.
        value: u64,
    },
}

impl fmt::Display for RegisterValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register schema: {self:?}")
    }
}

impl std::error::Error for RegisterValidationError {}

/// Contextual failure while validating an offset-free register definition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterDefinitionError<I = u16> {
    /// The byte width is unsupported.
    InvalidWidth {
        /// Root map or definition name.
        root: &'static str,
        /// Root-qualified placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Definition name.
        name: &'static str,
        /// Supplied width.
        width: usize,
    },
    /// The required byte alignment is zero or not a power of two.
    InvalidAlignment {
        /// Root map or definition name.
        root: &'static str,
        /// Root-qualified placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Definition name.
        name: &'static str,
        /// Supplied alignment.
        alignment: usize,
    },
    /// The writable mask exceeds the byte width.
    WritableMaskOverflow {
        /// Root name.
        root: &'static str,
        /// Placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Definition name.
        name: &'static str,
        /// Declared width.
        width: usize,
        /// Supplied mask.
        writable_mask: u64,
    },
    /// The reset value exceeds the byte width.
    ResetValueOverflow {
        /// Root name.
        root: &'static str,
        /// Placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Definition name.
        name: &'static str,
        /// Declared width.
        width: usize,
        /// Supplied reset value.
        reset_value: u64,
    },
    /// Two fields reuse an identity.
    DuplicateFieldId {
        /// Root name.
        root: &'static str,
        /// Later field path.
        path: String,
        /// Later field declaration index.
        declaration_index: usize,
        /// Repeated identity.
        id: I,
    },
    /// Two fields reuse a name.
    DuplicateFieldName {
        /// Root name.
        root: &'static str,
        /// Later field path.
        path: String,
        /// Later field declaration index.
        declaration_index: usize,
        /// Repeated name.
        name: &'static str,
    },
    /// A zero-width field or checked bit-position addition is invalid.
    FieldBitArithmetic {
        /// Root name.
        root: &'static str,
        /// Field path.
        path: String,
        /// Field declaration index.
        declaration_index: usize,
        /// Field identity.
        id: I,
        /// Least-significant bit.
        lsb: usize,
        /// Declared bit width.
        bit_width: usize,
    },
    /// A field extends beyond the register width.
    FieldOutOfBounds {
        /// Root name.
        root: &'static str,
        /// Field path.
        path: String,
        /// Field declaration index.
        declaration_index: usize,
        /// Field identity.
        id: I,
        /// Least-significant bit.
        lsb: usize,
        /// Declared bit width.
        bit_width: usize,
        /// Register width in bits.
        register_bits: usize,
    },
    /// Two field bit ranges intersect.
    FieldOverlap {
        /// Root name.
        root: &'static str,
        /// Definition placement path.
        path: String,
        /// Later field declaration index.
        declaration_index: usize,
        /// Earlier field identity.
        first_id: I,
        /// Later field identity.
        second_id: I,
    },
    /// Writable bits are not covered by any declared field.
    WritableMaskOutsideFields {
        /// Root name.
        root: &'static str,
        /// Definition placement path.
        path: String,
        /// Placement declaration index.
        declaration_index: usize,
        /// Declared writable mask.
        writable_mask: u64,
        /// Union of declared field masks.
        declared_field_mask: u64,
        /// Writable bits outside the field union.
        outside_mask: u64,
    },
}

impl<I: fmt::Debug> fmt::Display for RegisterDefinitionError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register definition: {self:?}")
    }
}

impl<I: fmt::Debug> std::error::Error for RegisterDefinitionError<I> {}

/// Failure while accessing a checked register-value view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterValueError {
    /// The definition cannot safely describe a checked value view.
    InvalidDefinition,
    /// The supplied byte slice is shorter than the register width.
    BufferTooShort {
        /// Required byte length.
        required: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// A whole-register value exceeds the declared width.
    ValueOverflow {
        /// Register width in bytes.
        width: usize,
        /// Supplied value.
        value: u64,
    },
    /// No field has the supplied name.
    UnknownFieldName {
        /// Supplied name.
        name: &'static str,
    },
    /// A field value exceeds its declared bit width.
    FieldValueOverflow {
        /// Exact schema member and root-relative bit coordinate.
        location: SchemaLocation,
        /// Field width in bits.
        bit_width: usize,
        /// Supplied value.
        value: u64,
    },
    /// A field location cannot be represented at its containing layout coordinate.
    CoordinateRebaseOverflow {
        /// Original register-local field location.
        location: SchemaLocation,
        /// Containing layout coordinate that could not be added.
        base: SchemaCoordinate,
    },
}

impl RegisterValueError {
    /// Rebases a field-bearing failure under `base` without changing its category.
    #[must_use]
    pub const fn rebased(self, base: SchemaCoordinate) -> Self {
        match self {
            Self::InvalidDefinition => Self::InvalidDefinition,
            Self::BufferTooShort { required, actual } => Self::BufferTooShort { required, actual },
            Self::ValueOverflow { width, value } => Self::ValueOverflow { width, value },
            Self::UnknownFieldName { name } => Self::UnknownFieldName { name },
            Self::FieldValueOverflow {
                location,
                bit_width,
                value,
            } => match location.checked_rebase(base) {
                Some(location) => Self::FieldValueOverflow {
                    location,
                    bit_width,
                    value,
                },
                None => Self::CoordinateRebaseOverflow { location, base },
            },
            Self::CoordinateRebaseOverflow { location, base } => {
                Self::CoordinateRebaseOverflow { location, base }
            }
        }
    }
}

impl fmt::Display for RegisterValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register value access: {self:?}")
    }
}

impl std::error::Error for RegisterValueError {}
