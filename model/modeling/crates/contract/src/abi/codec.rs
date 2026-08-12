//! Generated-view error conversion shared by ABI record codecs.

use hardware_interface_schema::SchemaCodecError;

use super::{AbiDecodeError, AbiEncodeError};

/// Returns the explicit little-endian bytes for a `u64`.
pub const fn copy_u64_le(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

pub(super) const fn decode_schema_error(error: SchemaCodecError) -> AbiDecodeError {
    match error {
        SchemaCodecError::BufferTooShort { required, actual } => {
            AbiDecodeError::BufferTooShort { required, actual }
        }
        _ => AbiDecodeError::InvalidRecordMetadata,
    }
}

pub(super) const fn encode_schema_error(error: SchemaCodecError) -> AbiEncodeError {
    match error {
        SchemaCodecError::BufferTooShort { required, actual } => {
            AbiEncodeError::BufferTooShort { required, actual }
        }
        _ => AbiEncodeError::InvalidRecordMetadata,
    }
}
