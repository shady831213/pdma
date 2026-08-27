//! Generated-view error conversion shared by ABI record codecs.

use hardware_interface_schema::SchemaCodecError;

use super::{AbiDecodeError, AbiEncodeError};

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
