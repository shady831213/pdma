use hardware_interface_schema::SchemaCodecError;

use crate::abi::{
    ABI_VERSION, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorSchemaPayloadValues, DescriptorSchemaValues, DescriptorSchemaView, DmaAddress,
    IndirectDestinationEntrySchemaValues, MAX_INDIRECT_DESTINATIONS, VECTOR_ADD_OPCODE,
    codec::decode_schema_error,
};

use super::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiStridedCopy, AbiVectorAdd, BroadcastDestinationEntry,
    BroadcastDmaAddresses, BroadcastShape, CopyDmaAddresses, CopyStrides, DescriptorDecodeError,
    DescriptorValueError, StridedCopyShape, VectorDmaAddresses,
};

pub(super) fn decode_descriptor(bytes: &[u8]) -> Result<AbiCommand, DescriptorDecodeError> {
    let view = DescriptorSchemaView::new(bytes).map_err(decode_schema_error)?;
    let header = view.header().map_err(decode_schema_error)?;
    let version = header.version().map_err(decode_schema_error)?;
    let size = header.size().map_err(decode_schema_error)?;
    let opcode = header.opcode().map_err(decode_schema_error)?;
    let flags = header.flags().map_err(decode_schema_error)?;
    validate_header(version, size, opcode, flags)?;
    let values = DescriptorSchemaValues::from_bytes(bytes)
        .map_err(|error| descriptor_schema_error(opcode, error))?;
    decode_variant(AbiId::new(values.header.command_id), values.payload)
}

pub(super) fn decode_destination(
    bytes: &[u8],
) -> Result<BroadcastDestinationEntry, DescriptorDecodeError> {
    let values =
        IndirectDestinationEntrySchemaValues::from_bytes(bytes).map_err(decode_schema_error)?;
    Ok(BroadcastDestinationEntry::new(DmaAddress::new(
        values.destination_dma_address,
    )))
}

fn validate_header(
    version: u16,
    size: u16,
    opcode: u16,
    flags: u16,
) -> Result<(), DescriptorDecodeError> {
    if version != ABI_VERSION {
        return Err(DescriptorDecodeError::InvalidVersion {
            observed: version,
            expected: ABI_VERSION,
        });
    }
    if size != DESCRIPTOR_SIZE {
        return Err(DescriptorDecodeError::InvalidSize {
            observed: size,
            expected: DESCRIPTOR_SIZE,
        });
    }
    if !(COPY_OPCODE..=VECTOR_ADD_OPCODE).contains(&opcode) {
        return Err(DescriptorDecodeError::InvalidOpcode {
            observed: opcode,
            maximum: VECTOR_ADD_OPCODE,
        });
    }
    if flags != DESCRIPTOR_FLAGS_MASK {
        return Err(DescriptorDecodeError::InvalidFlags {
            observed: flags,
            allowed: DESCRIPTOR_FLAGS_MASK,
        });
    }
    Ok(())
}

fn decode_variant(
    command_id: AbiId,
    variant: DescriptorSchemaPayloadValues,
) -> Result<AbiCommand, DescriptorDecodeError> {
    match variant {
        DescriptorSchemaPayloadValues::Copy {
            source_dma_address,
            destination_dma_address,
            length,
        } => Ok(AbiCommand::Copy(AbiCopy::new(
            command_id,
            CopyDmaAddresses::new(
                DmaAddress::new(source_dma_address),
                DmaAddress::new(destination_dma_address),
            ),
            length,
        ))),
        DescriptorSchemaPayloadValues::StridedCopy {
            source_dma_address,
            destination_dma_address,
            rows,
            row_bytes,
            source_stride,
            destination_stride,
        } => Ok(AbiCommand::StridedCopy(AbiStridedCopy::new(
            command_id,
            CopyDmaAddresses::new(
                DmaAddress::new(source_dma_address),
                DmaAddress::new(destination_dma_address),
            ),
            StridedCopyShape::new(
                rows,
                row_bytes,
                CopyStrides::new(source_stride, destination_stride),
            ),
        ))),
        DescriptorSchemaPayloadValues::BroadcastCopy {
            source_dma_address,
            destination_list_dma_address,
            length,
            destination_count,
        } => AbiBroadcastCopy::try_new(
            command_id,
            BroadcastDmaAddresses::new(
                DmaAddress::new(source_dma_address),
                DmaAddress::new(destination_list_dma_address),
            ),
            BroadcastShape::new(length, destination_count),
        )
        .map(AbiCommand::BroadcastCopy)
        .map_err(map_broadcast_error),
        DescriptorSchemaPayloadValues::VectorAdd {
            source_a_dma_address,
            source_b_dma_address,
            destination_dma_address,
            length,
        } => Ok(AbiCommand::VectorAdd(AbiVectorAdd::new(
            command_id,
            VectorDmaAddresses::new(
                DmaAddress::new(source_a_dma_address),
                DmaAddress::new(source_b_dma_address),
                DmaAddress::new(destination_dma_address),
            ),
            length,
        ))),
    }
}

fn map_broadcast_error(error: DescriptorValueError) -> DescriptorDecodeError {
    match error {
        DescriptorValueError::InvalidIndirectCount { observed, maximum } => {
            DescriptorDecodeError::InvalidIndirectCount {
                observed: u64::from(observed),
                maximum,
            }
        }
        DescriptorValueError::InvalidIndirectAlignment { address, alignment } => {
            DescriptorDecodeError::InvalidIndirectAlignment { address, alignment }
        }
    }
}

fn descriptor_schema_error(opcode: u16, error: SchemaCodecError) -> DescriptorDecodeError {
    match error {
        SchemaCodecError::BufferTooShort { .. } => {
            DescriptorDecodeError::Codec(decode_schema_error(error))
        }
        SchemaCodecError::UnknownTag { observed } => {
            let Ok(observed) = u16::try_from(observed) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            DescriptorDecodeError::InvalidOpcode {
                observed,
                maximum: VECTOR_ADD_OPCODE,
            }
        }
        SchemaCodecError::NonZeroReserved { offset, observed } => field_offset(offset)
            .map_or(DescriptorDecodeError::MetadataMismatch, |offset| {
                DescriptorDecodeError::InvalidReserved { offset, observed }
            }),
        SchemaCodecError::NarrowingOverflow {
            offset,
            observed,
            maximum,
        } => {
            if opcode == BROADCAST_COPY_OPCODE && offset == 40 {
                return DescriptorDecodeError::InvalidIndirectCount {
                    observed,
                    maximum: MAX_INDIRECT_DESTINATIONS,
                };
            }
            let Ok(offset) = field_offset(offset) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            let Ok(maximum) = u32::try_from(maximum) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            DescriptorDecodeError::ValueOutOfRange {
                offset,
                observed,
                maximum,
            }
        }
        _ => DescriptorDecodeError::MetadataMismatch,
    }
}

fn field_offset(offset: usize) -> Result<u16, DescriptorDecodeError> {
    u16::try_from(offset).map_err(|_| DescriptorDecodeError::MetadataMismatch)
}
