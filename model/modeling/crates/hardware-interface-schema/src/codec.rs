use core::fmt;

use crate::{Endian, Scalar};

/// Failure while reading or writing a fixed-width scalar.
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
    /// A record union contains an unassigned discriminant.
    UnknownTag {
        /// Observed discriminant value.
        observed: u64,
    },
    /// A record contains nonzero reserved storage.
    NonZeroReserved {
        /// Reserved scalar byte offset.
        offset: usize,
        /// Observed reserved scalar value.
        observed: u64,
    },
    /// A stored record scalar does not fit its semantic scalar type.
    NarrowingOverflow {
        /// Stored scalar byte offset.
        offset: usize,
        /// Observed stored value.
        observed: u64,
        /// Largest representable semantic value.
        maximum: u64,
    },
    /// A derived record field does not contain its required constant.
    ConstantMismatch {
        /// Derived scalar byte offset.
        offset: usize,
        /// Observed stored value.
        observed: u64,
        /// Required generated value.
        expected: u64,
    },
}

impl fmt::Display for SchemaCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => write!(
                formatter,
                "scalar buffer is too short: required {required} bytes, received {actual}"
            ),
            Self::UnknownTag { observed } => write!(formatter, "unknown record tag {observed}"),
            Self::NonZeroReserved { offset, observed } => write!(
                formatter,
                "reserved record storage at byte {offset} is {observed:#x}"
            ),
            Self::NarrowingOverflow {
                offset,
                observed,
                maximum,
            } => write!(
                formatter,
                "record value {observed} at byte {offset} exceeds {maximum}"
            ),
            Self::ConstantMismatch {
                offset,
                observed,
                expected,
            } => write!(
                formatter,
                "record constant at byte {offset} is {observed}, expected {expected}"
            ),
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
