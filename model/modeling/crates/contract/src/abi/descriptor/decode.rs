use crate::abi::{
    ABI_VERSION, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorSchema, DescriptorSchemaValues, DmaAddress, IndirectDestinationEntrySchemaValues,
    MAX_INDIRECT_DESTINATIONS, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES,
    STRIDED_COPY_OPCODE, VECTOR_ADD_OPCODE, codec::decode_schema_error,
};

use super::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiStridedCopy, AbiVectorAdd, BroadcastDestinationEntry,
    BroadcastIovas, BroadcastShape, CopyIovas, CopyStrides, DescriptorDecodeError,
    DescriptorValueError, StridedCopyShape, VectorIovas,
};

pub(super) fn decode_descriptor(bytes: &[u8]) -> Result<AbiCommand, DescriptorDecodeError> {
    let values = DescriptorSchemaValues::from_bytes(bytes).map_err(decode_schema_error)?;
    validate_header(values.version, values.size, values.opcode, values.flags)?;
    validate_reserved(values.opcode, &values)?;
    decode_payload(values.opcode, AbiId::new(values.command_id), &values)
}

pub(super) fn decode_destination(
    bytes: &[u8],
) -> Result<BroadcastDestinationEntry, DescriptorDecodeError> {
    let values =
        IndirectDestinationEntrySchemaValues::from_bytes(bytes).map_err(decode_schema_error)?;
    Ok(BroadcastDestinationEntry::new(DmaAddress::new(
        values.destination_iova,
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

fn validate_reserved(
    opcode: u16,
    payload: &DescriptorSchemaValues,
) -> Result<(), DescriptorDecodeError> {
    let reserved = match opcode {
        COPY_OPCODE => [
            Some((payload.word3, DescriptorSchema::WORD3_OFFSET)),
            Some((payload.word4, DescriptorSchema::WORD4_OFFSET)),
            Some((payload.word5, DescriptorSchema::WORD5_OFFSET)),
        ],
        BROADCAST_COPY_OPCODE | VECTOR_ADD_OPCODE => [
            Some((payload.word4, DescriptorSchema::WORD4_OFFSET)),
            Some((payload.word5, DescriptorSchema::WORD5_OFFSET)),
            None,
        ],
        STRIDED_COPY_OPCODE => [None, None, None],
        _ => {
            return Err(DescriptorDecodeError::InvalidOpcode {
                observed: opcode,
                maximum: VECTOR_ADD_OPCODE,
            });
        }
    };
    for (observed, field) in reserved.into_iter().flatten() {
        if observed != 0 {
            return Err(DescriptorDecodeError::InvalidReserved {
                offset: field_offset(field)?,
                observed,
            });
        }
    }
    Ok(())
}

fn decode_payload(
    opcode: u16,
    command_id: AbiId,
    payload: &DescriptorSchemaValues,
) -> Result<AbiCommand, DescriptorDecodeError> {
    match opcode {
        COPY_OPCODE => Ok(AbiCommand::Copy(AbiCopy::new(
            command_id,
            CopyIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
            ),
            bounded(
                payload.word2,
                DescriptorSchema::WORD2_OFFSET,
                MAX_TRANSFER_BYTES,
            )?,
        ))),
        STRIDED_COPY_OPCODE => Ok(AbiCommand::StridedCopy(AbiStridedCopy::new(
            command_id,
            CopyIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
            ),
            StridedCopyShape::new(
                bounded(
                    payload.word2,
                    DescriptorSchema::WORD2_OFFSET,
                    MAX_STRIDED_ROWS,
                )?,
                bounded(
                    payload.word3,
                    DescriptorSchema::WORD3_OFFSET,
                    MAX_TRANSFER_BYTES,
                )?,
                CopyStrides::new(
                    bounded(payload.word4, DescriptorSchema::WORD4_OFFSET, MAX_STRIDE)?,
                    bounded(payload.word5, DescriptorSchema::WORD5_OFFSET, MAX_STRIDE)?,
                ),
            ),
        ))),
        BROADCAST_COPY_OPCODE => decode_broadcast(command_id, payload),
        VECTOR_ADD_OPCODE => Ok(AbiCommand::VectorAdd(AbiVectorAdd::new(
            command_id,
            VectorIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
                DmaAddress::new(payload.word2),
            ),
            bounded(
                payload.word3,
                DescriptorSchema::WORD3_OFFSET,
                MAX_TRANSFER_BYTES,
            )?,
        ))),
        _ => Err(DescriptorDecodeError::InvalidOpcode {
            observed: opcode,
            maximum: VECTOR_ADD_OPCODE,
        }),
    }
}

fn decode_broadcast(
    command_id: AbiId,
    payload: &DescriptorSchemaValues,
) -> Result<AbiCommand, DescriptorDecodeError> {
    let length = bounded(
        payload.word2,
        DescriptorSchema::WORD2_OFFSET,
        MAX_TRANSFER_BYTES,
    )?;
    let count =
        u32::try_from(payload.word3).map_err(|_| DescriptorDecodeError::InvalidIndirectCount {
            observed: payload.word3,
            maximum: MAX_INDIRECT_DESTINATIONS,
        })?;
    AbiBroadcastCopy::try_new(
        command_id,
        BroadcastIovas::new(
            DmaAddress::new(payload.word0),
            DmaAddress::new(payload.word1),
        ),
        BroadcastShape::new(length, count),
    )
    .map(AbiCommand::BroadcastCopy)
    .map_err(|error| match error {
        DescriptorValueError::InvalidIndirectCount { observed, maximum } => {
            DescriptorDecodeError::InvalidIndirectCount {
                observed: u64::from(observed),
                maximum,
            }
        }
        DescriptorValueError::InvalidIndirectAlignment { address, alignment } => {
            DescriptorDecodeError::InvalidIndirectAlignment { address, alignment }
        }
    })
}

fn bounded(observed: u64, offset: usize, maximum: u32) -> Result<u32, DescriptorDecodeError> {
    let offset = field_offset(offset)?;
    u32::try_from(observed).map_err(|_| DescriptorDecodeError::ValueOutOfRange {
        offset,
        observed,
        maximum,
    })
}

fn field_offset(offset: usize) -> Result<u16, DescriptorDecodeError> {
    u16::try_from(offset).map_err(|_| DescriptorDecodeError::MetadataMismatch)
}
