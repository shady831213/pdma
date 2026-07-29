//! Checked little-endian scalar helpers shared by ABI record codecs.

use core::ops::Range;

use super::{AbiDecodeError, AbiEncodeError};

/// Returns the explicit little-endian bytes for a `u64`.
pub const fn copy_u64_le(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

/// Reads a little-endian `u16` from an exact checked range.
pub fn read_u16_le(bytes: &[u8], range: Range<usize>) -> Result<u16, AbiDecodeError> {
    let field = decode_field(bytes, range, 2)?;
    let encoded = <[u8; 2]>::try_from(field).map_err(|_| AbiDecodeError::InvalidFieldRange {
        start: 0,
        end: field.len(),
        width: 2,
    })?;
    Ok(u16::from_le_bytes(encoded))
}

pub(super) fn read_u32_le(bytes: &[u8], range: Range<usize>) -> Result<u32, AbiDecodeError> {
    let field = decode_field(bytes, range, 4)?;
    let encoded = <[u8; 4]>::try_from(field).map_err(|_| AbiDecodeError::InvalidFieldRange {
        start: 0,
        end: field.len(),
        width: 4,
    })?;
    Ok(u32::from_le_bytes(encoded))
}

/// Reads a little-endian `u64` from an exact checked range.
pub fn read_u64_le(bytes: &[u8], range: Range<usize>) -> Result<u64, AbiDecodeError> {
    let field = decode_field(bytes, range, 8)?;
    let encoded = <[u8; 8]>::try_from(field).map_err(|_| AbiDecodeError::InvalidFieldRange {
        start: 0,
        end: field.len(),
        width: 8,
    })?;
    Ok(u64::from_le_bytes(encoded))
}

/// Writes a little-endian `u16` into an exact checked range.
pub fn write_u16_le(
    bytes: &mut [u8],
    range: Range<usize>,
    value: u16,
) -> Result<(), AbiEncodeError> {
    encode_field(bytes, range, &value.to_le_bytes())
}

/// Writes a little-endian `u64` into an exact checked range.
pub fn write_u64_le(
    bytes: &mut [u8],
    range: Range<usize>,
    value: u64,
) -> Result<(), AbiEncodeError> {
    encode_field(bytes, range, &value.to_le_bytes())
}

fn decode_field(bytes: &[u8], range: Range<usize>, width: usize) -> Result<&[u8], AbiDecodeError> {
    if range.end < range.start || range.end - range.start != width {
        return Err(AbiDecodeError::InvalidFieldRange {
            start: range.start,
            end: range.end,
            width,
        });
    }
    bytes
        .get(range.clone())
        .ok_or(AbiDecodeError::BufferTooShort {
            required: range.end,
            actual: bytes.len(),
        })
}

fn encode_field(
    bytes: &mut [u8],
    range: Range<usize>,
    encoded: &[u8],
) -> Result<(), AbiEncodeError> {
    if range.end < range.start || range.end - range.start != encoded.len() {
        return Err(AbiEncodeError::InvalidFieldRange {
            start: range.start,
            end: range.end,
            width: encoded.len(),
        });
    }
    let actual = bytes.len();
    let field = bytes
        .get_mut(range.clone())
        .ok_or(AbiEncodeError::BufferTooShort {
            required: range.end,
            actual,
        })?;
    field.copy_from_slice(encoded);
    Ok(())
}
