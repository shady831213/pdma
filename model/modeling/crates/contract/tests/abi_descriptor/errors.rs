use super::{support, vectors};
use pdma_model_contract::abi::{
    AbiCommand, AbiDecodeError, AbiEncodeError, AbiId, BroadcastDestinationEntry,
    DescriptorDecodeError, DescriptorHeaderSchemaOpcode, DescriptorSchema,
    DescriptorSchemaPayloadBroadcastCopyValues, DescriptorSchemaPayloadCopyValues,
    DescriptorSchemaPayloadStridedCopyValues, DescriptorSchemaPayloadVectorAddValues,
    DescriptorValueError, DescriptorViolationKind, INDIRECT_ENTRY_ALIGNMENT,
    MAX_INDIRECT_DESTINATIONS, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES,
};

fn assert_decode_violation(bytes: &[u8], expected: DescriptorViolationKind) {
    let decoded = AbiCommand::decode(bytes);
    assert!(matches!(decoded, Err(DescriptorDecodeError::Violation(_))));
    let Err(DescriptorDecodeError::Violation(violation)) = decoded else {
        return;
    };
    assert_eq!(violation.kind(), expected);
}

/// Checks every operation-reserved word and bounded scalar.
///
/// # Panics
/// Panics when a malformed fixture is not rejected with its exact typed error.
pub fn assert_reserved_and_scalar_errors() {
    assert_reserved_errors();
    assert_scalar_errors();
    assert_reserved_precedence();
}

fn assert_reserved_errors() {
    for (base, fields) in [
        (
            vectors::COPY,
            &[
                (40_usize, DescriptorHeaderSchemaOpcode::Copy),
                (48, DescriptorHeaderSchemaOpcode::Copy),
                (56, DescriptorHeaderSchemaOpcode::Copy),
            ][..],
        ),
        (
            vectors::BROADCAST,
            &[
                (48_usize, DescriptorHeaderSchemaOpcode::BroadcastCopy),
                (56, DescriptorHeaderSchemaOpcode::BroadcastCopy),
            ][..],
        ),
        (
            vectors::VECTOR_ADD,
            &[
                (48_usize, DescriptorHeaderSchemaOpcode::VectorAdd),
                (56, DescriptorHeaderSchemaOpcode::VectorAdd),
            ][..],
        ),
    ] {
        for &(offset, opcode) in fields {
            let mut bytes = base;
            assert!(support::replace_u64(
                &mut bytes,
                offset,
                0x0102_0304_0506_0708
            ));
            assert_decode_violation(
                &bytes,
                DescriptorViolationKind::Reserved {
                    opcode,
                    location: DescriptorSchema::PAYLOAD_IDENTITY.at_byte(offset),
                },
            );
        }
    }
}

fn assert_scalar_errors() {
    for (base, offset, maximum, location) in [
        (
            vectors::COPY,
            32,
            MAX_TRANSFER_BYTES,
            DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            32,
            MAX_STRIDED_ROWS,
            DescriptorSchemaPayloadStridedCopyValues::ROWS_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            40,
            MAX_TRANSFER_BYTES,
            DescriptorSchemaPayloadStridedCopyValues::ROW_BYTES_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
        (
            vectors::STRIDED,
            48,
            MAX_STRIDE,
            DescriptorSchemaPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
                .at_byte(DescriptorSchema::WORD4_OFFSET),
        ),
        (
            vectors::STRIDED,
            56,
            MAX_STRIDE,
            DescriptorSchemaPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
                .at_byte(DescriptorSchema::WORD5_OFFSET),
        ),
        (
            vectors::BROADCAST,
            32,
            MAX_TRANSFER_BYTES,
            DescriptorSchemaPayloadBroadcastCopyValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::VECTOR_ADD,
            40,
            MAX_TRANSFER_BYTES,
            DescriptorSchemaPayloadVectorAddValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
    ] {
        let mut bytes = base;
        let observed = u64::from(maximum) + 1;
        assert!(support::replace_u64(&mut bytes, offset, observed));
        assert_decode_violation(&bytes, DescriptorViolationKind::Scalar(location));
    }
}

fn assert_reserved_precedence() {
    let mut reserved_and_scalar = vectors::COPY;
    assert!(support::replace_u64(
        &mut reserved_and_scalar,
        32,
        u64::from(u32::MAX) + 1
    ));
    assert!(support::replace_u64(
        &mut reserved_and_scalar,
        40,
        0x0102_0304_0506_0708
    ));
    assert_decode_violation(
        &reserved_and_scalar,
        DescriptorViolationKind::Reserved {
            opcode: DescriptorHeaderSchemaOpcode::Copy,
            location: DescriptorSchema::PAYLOAD_IDENTITY.at_byte(DescriptorSchema::WORD3_OFFSET),
        },
    );
}

/// Checks count, alignment, short slices, and host construction failures.
///
/// # Panics
/// Panics when a malformed boundary value is not rejected with its exact typed error.
pub fn assert_count_alignment_and_slice_errors() {
    assert_decode_count_alignment_errors();
    assert_short_slice_errors();
    assert_construction_errors();
}

fn assert_decode_count_alignment_errors() {
    let mut zero_count_and_misaligned = vectors::BROADCAST;
    assert!(support::replace_u64(
        &mut zero_count_and_misaligned,
        24,
        0x1003
    ));
    assert!(support::replace_u64(&mut zero_count_and_misaligned, 40, 0));
    assert_decode_violation(
        &zero_count_and_misaligned,
        DescriptorViolationKind::Scalar(
            DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
    );
    let mut zero_count = vectors::BROADCAST;
    assert!(support::replace_u64(&mut zero_count, 40, 0));
    assert_decode_violation(
        &zero_count,
        DescriptorViolationKind::Scalar(
            DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
    );
    let mut large_count = vectors::BROADCAST;
    assert!(support::replace_u64(
        &mut large_count,
        40,
        u64::from(u32::MAX) + 1
    ));
    assert_decode_violation(
        &large_count,
        DescriptorViolationKind::Scalar(
            DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
    );
    let mut misaligned = vectors::BROADCAST;
    assert!(support::replace_u64(&mut misaligned, 24, 0x1003));
    assert_decode_violation(
        &misaligned,
        DescriptorViolationKind::Scalar(
            DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
                .at_byte(DescriptorSchema::WORD1_OFFSET),
        ),
    );
}

fn assert_short_slice_errors() {
    assert_eq!(
        AbiCommand::decode(&vectors::COPY[..63]),
        Err(DescriptorDecodeError::Codec(
            AbiDecodeError::BufferTooShort {
                required: 64,
                actual: 63,
            }
        ))
    );
    assert_eq!(
        BroadcastDestinationEntry::decode(&vectors::DESTINATION_ENTRY[..7]),
        Err(DescriptorDecodeError::Codec(
            AbiDecodeError::BufferTooShort {
                required: 8,
                actual: 7,
            }
        ))
    );
    let mut short_output = [0_u8; 63];
    let commands = support::commands();
    assert!(commands.is_ok());
    let Ok([first, _, _, _]) = commands else {
        return;
    };
    assert_eq!(
        first.encode(&mut short_output),
        Err(AbiEncodeError::BufferTooShort {
            required: 64,
            actual: 63,
        })
    );
}

fn assert_construction_errors() {
    assert_eq!(
        AbiCommand::try_broadcast_copy(
            AbiId::new(1),
            DescriptorSchemaPayloadBroadcastCopyValues {
                source_dma_address: 2,
                destination_list_dma_address: 8,
                length: 0,
                destination_count: 0,
            },
        ),
        Err(DescriptorValueError::InvalidIndirectCount {
            observed: 0,
            maximum: MAX_INDIRECT_DESTINATIONS,
        })
    );
    assert_eq!(
        AbiCommand::try_broadcast_copy(
            AbiId::new(1),
            DescriptorSchemaPayloadBroadcastCopyValues {
                source_dma_address: 2,
                destination_list_dma_address: 11,
                length: 0,
                destination_count: 0,
            },
        ),
        Err(DescriptorValueError::InvalidIndirectCount {
            observed: 0,
            maximum: MAX_INDIRECT_DESTINATIONS,
        })
    );
    assert_eq!(
        AbiCommand::try_broadcast_copy(
            AbiId::new(1),
            DescriptorSchemaPayloadBroadcastCopyValues {
                source_dma_address: 2,
                destination_list_dma_address: 11,
                length: 0,
                destination_count: 1,
            },
        ),
        Err(DescriptorValueError::InvalidIndirectAlignment {
            address: 11,
            alignment: INDIRECT_ENTRY_ALIGNMENT,
        })
    );
}
