use hardware_interface_schema::SchemaCodecError;

use crate::abi::{DescriptorHeaderSchemaOpcode, DmaAddress, codec::decode_schema_error};

use super::super::{
    DescriptorDecodeError, DescriptorViolation, is_indirect_count_location, is_narrowing_location,
    is_reserved_location,
};

#[derive(Clone, Copy)]
pub(super) struct DescriptorContext {
    opcode: DescriptorHeaderSchemaOpcode,
    list: DmaAddress,
}

impl DescriptorContext {
    pub(super) const fn new(opcode: DescriptorHeaderSchemaOpcode, list: DmaAddress) -> Self {
        Self { opcode, list }
    }
}

pub(super) fn descriptor_schema_error(
    context: DescriptorContext,
    error: SchemaCodecError,
) -> DescriptorDecodeError {
    match error {
        SchemaCodecError::BufferTooShort { .. } => {
            DescriptorDecodeError::Codec(decode_schema_error(error))
        }
        SchemaCodecError::NonZeroReserved { location, observed } => {
            if is_reserved_location(context.opcode, location) {
                DescriptorDecodeError::Violation(DescriptorViolation::reserved(
                    context.opcode,
                    location,
                    observed,
                ))
            } else {
                DescriptorDecodeError::MetadataMismatch
            }
        }
        SchemaCodecError::NarrowingOverflow {
            location,
            observed,
            maximum,
        } => {
            if maximum != u64::from(u32::MAX) {
                return DescriptorDecodeError::MetadataMismatch;
            }
            if is_indirect_count_location(location) {
                return DescriptorDecodeError::Violation(DescriptorViolation::indirect_count(
                    context.list,
                    observed,
                ));
            }
            if is_narrowing_location(location) {
                DescriptorDecodeError::Violation(DescriptorViolation::scalar(location, observed))
            } else {
                DescriptorDecodeError::MetadataMismatch
            }
        }
        SchemaCodecError::UnknownTag { .. }
        | SchemaCodecError::ConstantMismatch { .. }
        | SchemaCodecError::CoordinateRebaseOverflow { .. } => {
            DescriptorDecodeError::MetadataMismatch
        }
        _ => DescriptorDecodeError::MetadataMismatch,
    }
}
