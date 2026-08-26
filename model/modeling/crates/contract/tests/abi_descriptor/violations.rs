use super::{support, vectors};
use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFaultCandidate,
    AbiFaultOrder, DESCRIPTOR_SIZE, DescriptorDecodeError, DescriptorHeaderSchema,
    DescriptorHeaderSchemaOpcode, DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadStridedCopyValues,
    DescriptorSchemaPayloadVectorAddValues, DescriptorViolationKind, DmaAddress,
    INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS, MAX_TRANSFER_BYTES, VECTOR_ADD_OPCODE,
};

const SLOT: DmaAddress = DmaAddress::new(0x4_000);
const LIST: DmaAddress = DmaAddress::new(0x8_800);
const TOO_WIDE: u64 = 0x1_0000_0000;

type FaultTuple = (AbiEventKind, u64, u64, u64, u16, u16, u64);

#[derive(Clone, Copy)]
struct Expected {
    kind: DescriptorViolationKind,
    tuple: FaultTuple,
}

#[test]
fn header_violations_map_to_canonical_hardware_faults() {
    // Given: every malformed fixed descriptor header field.
    let inputs = [
        (
            0,
            ABI_VERSION + 1,
            AbiEventKind::InvalidVersion,
            DescriptorHeaderSchema::VERSION_IDENTITY
                .at_byte(DescriptorHeaderSchema::VERSION_OFFSET),
            u64::from(ABI_VERSION),
        ),
        (
            6,
            DESCRIPTOR_SIZE - 1,
            AbiEventKind::InvalidSize,
            DescriptorHeaderSchema::SIZE_IDENTITY.at_byte(DescriptorHeaderSchema::SIZE_OFFSET),
            u64::from(DESCRIPTOR_SIZE),
        ),
        (
            2,
            VECTOR_ADD_OPCODE + 1,
            AbiEventKind::InvalidOpcode,
            DescriptorHeaderSchema::OPCODE_IDENTITY.at_byte(DescriptorHeaderSchema::OPCODE_OFFSET),
            u64::from(VECTOR_ADD_OPCODE),
        ),
        (
            4,
            1,
            AbiEventKind::InvalidFlags,
            DescriptorHeaderSchema::FLAGS_IDENTITY.at_byte(DescriptorHeaderSchema::FLAGS_OFFSET),
            0,
        ),
    ];

    // When/Then: decoding classifies each field and preserves its exact fault tuple.
    for (offset, observed, event_kind, location, expected) in inputs {
        let mut bytes = vectors::COPY;
        assert!(support::replace_u16(&mut bytes, offset, observed));
        let offset = u64::try_from(offset);
        assert!(offset.is_ok());
        let Ok(offset) = offset else {
            return;
        };
        assert_violation(
            &bytes,
            Expected {
                kind: DescriptorViolationKind::Header(location),
                tuple: descriptor_tuple(DescriptorExpected {
                    kind: event_kind,
                    observed: u64::from(observed),
                    expected,
                    offset,
                }),
            },
        );
    }
}

#[test]
fn reserved_violations_map_by_typed_operation_field() {
    // Given: every operation-specific required-zero descriptor word.
    let inputs = [
        (vectors::COPY, 40, DescriptorHeaderSchemaOpcode::Copy),
        (vectors::COPY, 48, DescriptorHeaderSchemaOpcode::Copy),
        (vectors::COPY, 56, DescriptorHeaderSchemaOpcode::Copy),
        (
            vectors::BROADCAST,
            48,
            DescriptorHeaderSchemaOpcode::BroadcastCopy,
        ),
        (
            vectors::BROADCAST,
            56,
            DescriptorHeaderSchemaOpcode::BroadcastCopy,
        ),
        (
            vectors::VECTOR_ADD,
            48,
            DescriptorHeaderSchemaOpcode::VectorAdd,
        ),
        (
            vectors::VECTOR_ADD,
            56,
            DescriptorHeaderSchemaOpcode::VectorAdd,
        ),
    ];

    // When/Then: equal offsets in different operations retain distinct typed identities.
    for (template, offset, opcode) in inputs {
        let mut bytes = template;
        assert!(support::replace_u64(&mut bytes, offset, 1));
        let location = DescriptorSchema::PAYLOAD_IDENTITY.at_byte(offset);
        let offset = u64::try_from(offset);
        assert!(offset.is_ok());
        let Ok(offset) = offset else {
            return;
        };
        assert_violation(
            &bytes,
            Expected {
                kind: DescriptorViolationKind::Reserved { opcode, location },
                tuple: descriptor_tuple(DescriptorExpected {
                    kind: AbiEventKind::InvalidReserved,
                    observed: 1,
                    expected: 0,
                    offset,
                }),
            },
        );
    }
}

#[test]
fn scalar_violations_map_by_generated_leaf_identity() {
    // Given: every ordinary fixed-width payload scalar stored above its semantic width.
    let inputs = [
        (
            vectors::COPY,
            32,
            DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            32,
            DescriptorSchemaPayloadStridedCopyValues::ROWS_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            40,
            DescriptorSchemaPayloadStridedCopyValues::ROW_BYTES_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
        (
            vectors::STRIDED,
            48,
            DescriptorSchemaPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
                .at_byte(DescriptorSchema::WORD4_OFFSET),
        ),
        (
            vectors::STRIDED,
            56,
            DescriptorSchemaPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
                .at_byte(DescriptorSchema::WORD5_OFFSET),
        ),
        (
            vectors::BROADCAST,
            32,
            DescriptorSchemaPayloadBroadcastCopyValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD2_OFFSET),
        ),
        (
            vectors::VECTOR_ADD,
            40,
            DescriptorSchemaPayloadVectorAddValues::LENGTH_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
    ];

    // When/Then: decoding maps the generated leaf to the exact scalar fault.
    for (template, offset, location) in inputs {
        let mut bytes = template;
        assert!(support::replace_u64(&mut bytes, offset, TOO_WIDE));
        let offset = u64::try_from(offset);
        assert!(offset.is_ok());
        let Ok(offset) = offset else {
            return;
        };
        assert_violation(
            &bytes,
            Expected {
                kind: DescriptorViolationKind::Scalar(location),
                tuple: descriptor_tuple(DescriptorExpected {
                    kind: AbiEventKind::ValueOutOfRange,
                    observed: TOO_WIDE,
                    expected: u64::from(MAX_TRANSFER_BYTES),
                    offset,
                }),
            },
        );
    }
}

#[test]
fn indirect_violations_embed_the_list_locator_and_preserve_precedence() {
    // Given: oversized, zero-plus-misaligned, and otherwise-valid misaligned broadcast lists.
    let mut oversized = vectors::BROADCAST;
    assert!(support::replace_u64(&mut oversized, 24, LIST.raw()));
    assert!(support::replace_u64(&mut oversized, 40, TOO_WIDE));
    let unaligned = DmaAddress::new(LIST.raw() + 3);
    let mut zero = vectors::BROADCAST;
    assert!(support::replace_u64(&mut zero, 24, unaligned.raw()));
    assert!(support::replace_u64(&mut zero, 40, 0));
    let mut alignment = vectors::BROADCAST;
    assert!(support::replace_u64(&mut alignment, 24, unaligned.raw()));

    // When/Then: count wins before alignment and every fault points at the encoded list.
    assert_violation(&oversized, indirect_count_expected(LIST, TOO_WIDE));
    assert_violation(&zero, indirect_count_expected(unaligned, 0));
    assert_violation(
        &alignment,
        Expected {
            kind: DescriptorViolationKind::Scalar(
                DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
                    .at_byte(DescriptorSchema::WORD1_OFFSET),
            ),
            tuple: (
                AbiEventKind::InvalidIndirectAlignment,
                unaligned.raw(),
                unaligned.raw() % INDIRECT_ENTRY_ALIGNMENT,
                INDIRECT_ENTRY_ALIGNMENT,
                AbiEventStage::IndirectList.value(),
                AbiEventAccess::None.value(),
                0,
            ),
        },
    );
}

fn assert_violation(bytes: &[u8], expected: Expected) {
    let decoded = AbiCommand::decode(bytes);
    assert!(matches!(decoded, Err(DescriptorDecodeError::Violation(_))));
    let Err(DescriptorDecodeError::Violation(violation)) = decoded else {
        return;
    };
    assert_eq!(violation.kind(), expected.kind);
    let fault = violation.into_abi_fault(SLOT);
    assert_eq!(fault.tuple(), expected.tuple);
    assert!(AbiFaultCandidate::try_new(fault, AbiFaultOrder::Descriptor(expected.kind)).is_ok());
}

fn indirect_count_expected(list: DmaAddress, observed: u64) -> Expected {
    Expected {
        kind: DescriptorViolationKind::Scalar(
            DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(DescriptorSchema::WORD3_OFFSET),
        ),
        tuple: (
            AbiEventKind::InvalidIndirectCount,
            list.raw(),
            observed,
            u64::from(MAX_INDIRECT_DESTINATIONS),
            AbiEventStage::IndirectList.value(),
            AbiEventAccess::None.value(),
            0,
        ),
    }
}

#[derive(Clone, Copy)]
struct DescriptorExpected {
    kind: AbiEventKind,
    observed: u64,
    expected: u64,
    offset: u64,
}

const fn descriptor_tuple(expected: DescriptorExpected) -> FaultTuple {
    (
        expected.kind,
        SLOT.raw(),
        expected.observed,
        expected.expected,
        AbiEventStage::Descriptor.value(),
        AbiEventAccess::None.value(),
        expected.offset,
    )
}
