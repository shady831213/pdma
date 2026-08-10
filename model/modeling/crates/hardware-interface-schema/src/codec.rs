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
}

impl fmt::Display for SchemaCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => write!(
                formatter,
                "scalar buffer is too short: required {required} bytes, received {actual}"
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
