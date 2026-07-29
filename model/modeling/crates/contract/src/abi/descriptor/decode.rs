use crate::abi::{
    ABI_VERSION, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR, DESCRIPTOR_FLAGS_MASK,
    DmaAddress, FieldSpec, MAX_INDIRECT_DESTINATIONS, MAX_STRIDE, MAX_STRIDED_ROWS,
    MAX_TRANSFER_BYTES, STRIDED_COPY_OPCODE, VECTOR_ADD_OPCODE, read_u16_le, read_u64_le,
};

use super::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiStridedCopy, AbiVectorAdd, BroadcastDestinationEntry,
    BroadcastIovas, BroadcastShape, CopyIovas, CopyStrides, DescriptorDecodeError,
    DescriptorValueError, StridedCopyShape, VectorIovas, codec::descriptor_fields,
};

struct Payload {
    word0: u64,
    word1: u64,
    word2: u64,
    word3: u64,
    word4: u64,
    word5: u64,
}

pub(super) fn decode_descriptor(bytes: &[u8]) -> Result<AbiCommand, DescriptorDecodeError> {
    let fields = descriptor_fields()?;
    let version = read_u16_le(bytes, fields.version.range())?;
    let opcode = read_u16_le(bytes, fields.opcode.range())?;
    let flags = read_u16_le(bytes, fields.flags.range())?;
    let size = read_u16_le(bytes, fields.size.range())?;
    let command_id = AbiId::new(read_u64_le(bytes, fields.command_id.range())?);
    let [word0, word1, word2, word3, word4, word5] = fields.words;
    let payload = Payload {
        word0: read_u64_le(bytes, word0.range())?,
        word1: read_u64_le(bytes, word1.range())?,
        word2: read_u64_le(bytes, word2.range())?,
        word3: read_u64_le(bytes, word3.range())?,
        word4: read_u64_le(bytes, word4.range())?,
        word5: read_u64_le(bytes, word5.range())?,
    };
    validate_header(version, size, opcode, flags)?;
    validate_reserved(opcode, &payload, [word3, word4, word5])?;
    decode_payload(opcode, command_id, &payload, [word2, word3, word4, word5])
}

pub(super) fn decode_destination(
    bytes: &[u8],
) -> Result<BroadcastDestinationEntry, DescriptorDecodeError> {
    Ok(BroadcastDestinationEntry::new(DmaAddress::new(
        read_u64_le(bytes, 0..8)?,
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
    if size != DESCRIPTOR.size() {
        return Err(DescriptorDecodeError::InvalidSize {
            observed: size,
            expected: DESCRIPTOR.size(),
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
    payload: &Payload,
    [word3, word4, word5]: [FieldSpec; 3],
) -> Result<(), DescriptorDecodeError> {
    let reserved = match opcode {
        COPY_OPCODE => [
            Some((payload.word3, word3)),
            Some((payload.word4, word4)),
            Some((payload.word5, word5)),
        ],
        BROADCAST_COPY_OPCODE | VECTOR_ADD_OPCODE => [
            Some((payload.word4, word4)),
            Some((payload.word5, word5)),
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
                offset: field.offset(),
                observed,
            });
        }
    }
    Ok(())
}

fn decode_payload(
    opcode: u16,
    command_id: AbiId,
    payload: &Payload,
    [word2, word3, word4, word5]: [FieldSpec; 4],
) -> Result<AbiCommand, DescriptorDecodeError> {
    match opcode {
        COPY_OPCODE => Ok(AbiCommand::Copy(AbiCopy::new(
            command_id,
            CopyIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
            ),
            bounded(payload.word2, word2, MAX_TRANSFER_BYTES)?,
        ))),
        STRIDED_COPY_OPCODE => Ok(AbiCommand::StridedCopy(AbiStridedCopy::new(
            command_id,
            CopyIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
            ),
            StridedCopyShape::new(
                bounded(payload.word2, word2, MAX_STRIDED_ROWS)?,
                bounded(payload.word3, word3, MAX_TRANSFER_BYTES)?,
                CopyStrides::new(
                    bounded(payload.word4, word4, MAX_STRIDE)?,
                    bounded(payload.word5, word5, MAX_STRIDE)?,
                ),
            ),
        ))),
        BROADCAST_COPY_OPCODE => decode_broadcast(command_id, payload, word2),
        VECTOR_ADD_OPCODE => Ok(AbiCommand::VectorAdd(AbiVectorAdd::new(
            command_id,
            VectorIovas::new(
                DmaAddress::new(payload.word0),
                DmaAddress::new(payload.word1),
                DmaAddress::new(payload.word2),
            ),
            bounded(payload.word3, word3, MAX_TRANSFER_BYTES)?,
        ))),
        _ => Err(DescriptorDecodeError::InvalidOpcode {
            observed: opcode,
            maximum: VECTOR_ADD_OPCODE,
        }),
    }
}

fn decode_broadcast(
    command_id: AbiId,
    payload: &Payload,
    length_field: FieldSpec,
) -> Result<AbiCommand, DescriptorDecodeError> {
    let length = bounded(payload.word2, length_field, MAX_TRANSFER_BYTES)?;
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

fn bounded(observed: u64, field: FieldSpec, maximum: u32) -> Result<u32, DescriptorDecodeError> {
    u32::try_from(observed).map_err(|_| DescriptorDecodeError::ValueOutOfRange {
        offset: field.offset(),
        observed,
        maximum,
    })
}
