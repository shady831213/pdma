use core::fmt;

use crate::{Endian, Scalar};

mod location;

pub use location::{SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};

/// Converts one typed raw representation into its semantic value.
pub trait Decode: Sized {
    /// Semantic value produced by decoding.
    type Decoded;

    /// Decodes this raw representation.
    ///
    /// # Errors
    /// Returns a typed codec failure when the raw representation is invalid.
    fn decode(self) -> Result<Self::Decoded, SchemaCodecError>;
}

/// Converts one semantic value into its typed raw representation.
pub trait Encode: Sized {
    /// Typed raw representation produced by encoding.
    type Encoded;

    /// Encodes this semantic value.
    ///
    /// # Errors
    /// Returns a typed codec failure when the semantic value cannot be represented.
    fn encode(self) -> Result<Self::Encoded, SchemaCodecError>;
}

/// Failure while converting raw schema storage and typed values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaCodecError {
    /// The supplied byte slice cannot hold the scalar.
    BufferTooShort {
        /// Minimum required byte length.
        required: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// A tagged schema member contains an unassigned discriminant.
    UnknownTag {
        /// Declared leaf member and root-relative coordinate.
        location: SchemaLocation,
        /// Observed discriminant value.
        observed: u64,
    },
    /// A schema member contains nonzero reserved storage.
    NonZeroReserved {
        /// Declared leaf member and root-relative coordinate.
        location: SchemaLocation,
        /// Observed reserved scalar value.
        observed: u64,
    },
    /// A stored scalar does not fit its semantic value type.
    NarrowingOverflow {
        /// Declared leaf member and root-relative coordinate.
        location: SchemaLocation,
        /// Observed stored value.
        observed: u64,
        /// Largest representable semantic value.
        maximum: u64,
    },
    /// A derived schema member does not contain its required constant.
    ConstantMismatch {
        /// Declared leaf member and root-relative coordinate.
        location: SchemaLocation,
        /// Observed stored value.
        observed: u64,
        /// Required generated value.
        expected: u64,
    },
    /// A nested schema location cannot be represented at its parent coordinate.
    CoordinateRebaseOverflow {
        /// Original child-local member location.
        location: SchemaLocation,
        /// Parent coordinate that could not be added.
        base: SchemaCoordinate,
    },
}

impl SchemaCodecError {
    /// Rebases a member-bearing failure under `base` without changing its category.
    #[must_use]
    pub const fn rebased(self, base: SchemaCoordinate) -> Self {
        match self {
            Self::BufferTooShort { required, actual } => Self::BufferTooShort { required, actual },
            Self::UnknownTag { location, observed } => {
                let Some(rebased) = location.checked_rebase(base) else {
                    return Self::CoordinateRebaseOverflow { location, base };
                };
                Self::UnknownTag {
                    location: rebased,
                    observed,
                }
            }
            Self::NonZeroReserved { location, observed } => {
                let Some(rebased) = location.checked_rebase(base) else {
                    return Self::CoordinateRebaseOverflow { location, base };
                };
                Self::NonZeroReserved {
                    location: rebased,
                    observed,
                }
            }
            Self::NarrowingOverflow {
                location,
                observed,
                maximum,
            } => {
                let Some(rebased) = location.checked_rebase(base) else {
                    return Self::CoordinateRebaseOverflow { location, base };
                };
                Self::NarrowingOverflow {
                    location: rebased,
                    observed,
                    maximum,
                }
            }
            Self::ConstantMismatch {
                location,
                observed,
                expected,
            } => {
                let Some(rebased) = location.checked_rebase(base) else {
                    return Self::CoordinateRebaseOverflow { location, base };
                };
                Self::ConstantMismatch {
                    location: rebased,
                    observed,
                    expected,
                }
            }
            Self::CoordinateRebaseOverflow { location, base } => {
                Self::CoordinateRebaseOverflow { location, base }
            }
        }
    }
}

impl fmt::Display for SchemaCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => write!(
                formatter,
                "schema buffer is too short: required {required} bytes, received {actual}"
            ),
            Self::UnknownTag { location, observed } => write!(
                formatter,
                "schema member {}.{} at {} has unknown tag {observed}",
                location.identity().owner(),
                location.identity().member(),
                CoordinateDisplay(location.coordinate())
            ),
            Self::NonZeroReserved { location, observed } => write!(
                formatter,
                "schema member {}.{} at {} has nonzero reserved storage {observed:#x}",
                location.identity().owner(),
                location.identity().member(),
                CoordinateDisplay(location.coordinate())
            ),
            Self::NarrowingOverflow {
                location,
                observed,
                maximum,
            } => write!(
                formatter,
                "schema member {}.{} at {} has value {observed} above {maximum}",
                location.identity().owner(),
                location.identity().member(),
                CoordinateDisplay(location.coordinate())
            ),
            Self::ConstantMismatch {
                location,
                observed,
                expected,
            } => write!(
                formatter,
                "schema member {}.{} at {} has constant {observed}, expected {expected}",
                location.identity().owner(),
                location.identity().member(),
                CoordinateDisplay(location.coordinate())
            ),
            Self::CoordinateRebaseOverflow { location, base } => write!(
                formatter,
                "schema member {}.{} at {} cannot be rebased under {}",
                location.identity().owner(),
                location.identity().member(),
                CoordinateDisplay(location.coordinate()),
                CoordinateDisplay(*base)
            ),
        }
    }
}

struct CoordinateDisplay(SchemaCoordinate);

impl fmt::Display for CoordinateDisplay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SchemaCoordinate::Byte(offset) => write!(formatter, "byte {offset}"),
            SchemaCoordinate::Bit(offset) => write!(formatter, "bit {offset}"),
        }
    }
}

impl std::error::Error for SchemaCodecError {}

/// Writes one supported scalar into the start of `output` using `endian`.
pub fn write_scalar<T: Scalar>(
    output: &mut [u8],
    endian: Endian,
    value: T,
) -> Result<(), SchemaCodecError> {
    value.write_to(output, endian)
}

/// Reads one supported scalar from the start of `input` using `endian`.
pub fn read_scalar<T: Scalar>(input: &[u8], endian: Endian) -> Result<T, SchemaCodecError> {
    T::read_from(input, endian)
}
