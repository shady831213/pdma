//! Checked little-endian scalar helpers shared by ABI record codecs.

use core::ops::Range;

use hardware_interface_schema::{Endian, SchemaCodecError, read_scalar, write_scalar};

use super::{AbiDecodeError, AbiEncodeError};

/// Returns the explicit little-endian bytes for a `u64`.
pub const fn copy_u64_le(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

/// Reads a little-endian `u16` from an exact checked range.
pub fn read_u16_le(bytes: &[u8], range: Range<usize>) -> Result<u16, AbiDecodeError> {
    read_le(bytes, range, 2)
}

pub(super) fn read_u32_le(bytes: &[u8], range: Range<usize>) -> Result<u32, AbiDecodeError> {
    read_le(bytes, range, 4)
}

/// Reads a little-endian `u64` from an exact checked range.
pub fn read_u64_le(bytes: &[u8], range: Range<usize>) -> Result<u64, AbiDecodeError> {
    read_le(bytes, range, 8)
}

/// Writes a little-endian `u16` into an exact checked range.
pub fn write_u16_le(
    bytes: &mut [u8],
    range: Range<usize>,
    value: u16,
) -> Result<(), AbiEncodeError> {
    write_le(bytes, range, 2, value)
}

/// Writes a little-endian `u64` into an exact checked range.
pub fn write_u64_le(
    bytes: &mut [u8],
    range: Range<usize>,
    value: u64,
) -> Result<(), AbiEncodeError> {
    write_le(bytes, range, 8, value)
}

fn read_le<T: hardware_interface_schema::Scalar>(
    bytes: &[u8],
    range: Range<usize>,
    width: usize,
) -> Result<T, AbiDecodeError> {
    if range.end < range.start || range.end - range.start != width {
        return Err(AbiDecodeError::InvalidFieldRange {
            start: range.start,
            end: range.end,
            width,
        });
    }
    let Some(tail) = bytes.get(range.start..) else {
        return Err(AbiDecodeError::BufferTooShort {
            required: range.end,
            actual: bytes.len(),
        });
    };
    read_scalar(tail, Endian::Little).map_err(|error| decode_error(error, range.end, bytes.len()))
}

fn write_le<T: hardware_interface_schema::Scalar>(
    bytes: &mut [u8],
    range: Range<usize>,
    width: usize,
    value: T,
) -> Result<(), AbiEncodeError> {
    if range.end < range.start || range.end - range.start != width {
        return Err(AbiEncodeError::InvalidFieldRange {
            start: range.start,
            end: range.end,
            width,
        });
    }
    let actual = bytes.len();
    let Some(tail) = bytes.get_mut(range.start..) else {
        return Err(AbiEncodeError::BufferTooShort {
            required: range.end,
            actual,
        });
    };
    write_scalar(tail, Endian::Little, value)
        .map_err(|error| encode_error(error, range.end, actual))
}

const fn decode_error(error: SchemaCodecError, required: usize, actual: usize) -> AbiDecodeError {
    match error {
        SchemaCodecError::BufferTooShort { .. } => {
            AbiDecodeError::BufferTooShort { required, actual }
        }
        _ => AbiDecodeError::BufferTooShort { required, actual },
    }
}

const fn encode_error(error: SchemaCodecError, required: usize, actual: usize) -> AbiEncodeError {
    match error {
        SchemaCodecError::BufferTooShort { .. } => {
            AbiEncodeError::BufferTooShort { required, actual }
        }
        _ => AbiEncodeError::BufferTooShort { required, actual },
    }
}
