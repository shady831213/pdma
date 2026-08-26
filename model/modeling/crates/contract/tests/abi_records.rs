//! ABI-v1 completion and event record boundary scenarios.

use hardware_interface_schema::{SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};
use pdma_model_contract::abi::{
    AbiBounds, AbiDecodeError, AbiEncodeError, AbiEventAccess, AbiEventKind, AbiEventStage,
    AbiFault, AbiFaultCandidate, AbiFaultDetails, AbiFaultOrder, AbiFaultSelectionError,
    AbiFaultSite, AbiOperandFaultStep, AbiOperandVisit, AbiStrideOffset, DescriptorHeaderSchema,
    DescriptorHeaderSchemaOpcode, DescriptorSchema, DescriptorSchemaPayloadCopyValues,
    DescriptorSchemaPayloadStridedCopyValues, DescriptorViolationKind, DmaAddress,
    select_canonical_fault,
};
use pdma_model_contract::binding::AbiEvent;
use pdma_model_contract::semantic::{CommandId, CompletionRecord, CompletionStatus, QueueId};
use pdma_model_contract::translation::{
    decode_completion, decode_event, encode_completion, encode_event,
};

include!("abi_records/fixtures.rs");
include!("abi_records/ordering.rs");
include!("abi_records/ordering_operands.rs");
include!("abi_records/ordering_helpers.rs");

#[test]
fn completion_and_all_event_kinds_match_golden_bytes() {
    // Given: one semantic success and canonical fixtures for all eleven event kinds.
    let completion = CompletionRecord::new(
        QueueId::new(0x1122),
        CommandId::new(0x3344),
        CompletionStatus::Success,
    );
    let events = canonical_events();

    // When: the records cross the ABI-v1 byte boundary.
    let completion_bytes = encode_completion(completion).expect("completion must encode");
    let event_bytes = events.map(|event| encode_event(event).expect("canonical event must encode"));
    let golden_tuples = [
        (1, 0x2000, 0x20, 0, 3, 1, 0),
        (2, u64::MAX - 7, 16, 0, 4, 2, 0),
        (3, 0x3030, 0x30, 0x3040, 4, 2, 0),
        (4, 0x1000, 2, 1, 1, 0, 0),
        (5, 0x1000, 63, 64, 1, 0, 6),
        (6, 0x1000, 9, 4, 1, 0, 2),
        (7, 0x1000, 1, 0, 1, 0, 4),
        (8, 0x1000, 1, 0, 1, 0, 40),
        (9, 0x4000, 0, u64::from(u32::MAX), 2, 0, 0),
        (10, 0x4003, 3, 8, 2, 0, 0),
        (
            11,
            0x1000,
            u64::from(u32::MAX) + 1,
            u64::from(u32::MAX),
            1,
            0,
            32,
        ),
    ];

    // Then: fixed-width bytes and decoded semantic values match independent fixtures.
    let mut completion_golden = [0_u8; 32];
    completion_golden[0..2].copy_from_slice(&1_u16.to_le_bytes());
    completion_golden[2..4].copy_from_slice(&32_u16.to_le_bytes());
    completion_golden[8..16].copy_from_slice(&0x1122_u64.to_le_bytes());
    completion_golden[16..24].copy_from_slice(&0x3344_u64.to_le_bytes());
    assert_eq!(completion_bytes, completion_golden);
    assert_eq!(
        decode_completion(&completion_bytes).expect("golden completion must decode"),
        completion
    );
    for ((encoded, expected), golden) in event_bytes.into_iter().zip(events).zip(golden_tuples) {
        assert_eq!(encoded, event_golden(golden));
        assert_eq!(
            decode_event(&encoded).expect("golden event must decode"),
            expected
        );
        println!("event {:?}: {:02x?}", expected.fault().kind(), encoded);
    }
}

#[test]
fn rejects_unknown_values_and_noncanonical_details() {
    // Given: malformed completion/event bytes.
    let mut completion = [0_u8; 32];
    completion[0..2].copy_from_slice(&1_u16.to_le_bytes());
    completion[2..4].copy_from_slice(&32_u16.to_le_bytes());
    completion[4..6].copy_from_slice(&9_u16.to_le_bytes());
    let mut event = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    event[4..6].copy_from_slice(&99_u16.to_le_bytes());
    let mut completion_flags = completion;
    completion_flags[4..6].copy_from_slice(&0_u16.to_le_bytes());
    completion_flags[6..8].copy_from_slice(&1_u16.to_le_bytes());
    let mut completion_reserved = completion_flags;
    completion_reserved[6..8].copy_from_slice(&0_u16.to_le_bytes());
    completion_reserved[24] = 1;
    let mut event_flags = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    event_flags[6] = 1;
    let mut event_reserved = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    event_reserved[52] = 1;
    let mut event_stage = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    event_stage[48..50].copy_from_slice(&99_u16.to_le_bytes());
    let mut event_access = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    event_access[50..52].copy_from_slice(&99_u16.to_le_bytes());

    // When: values that are not canonical ABI-v1 records cross the boundary.
    let unknown_status = decode_completion(&completion);
    let unknown_kind = decode_event(&event);
    let noncanonical = encode_event(AbiEvent::new(
        QueueId::new(1),
        CommandId::new(2),
        AbiFault::from_parts(
            AbiEventKind::UnmappedDmaAddress,
            DmaAddress::new(0x4000),
            AbiFaultDetails::new(8, 9, site(AbiEventStage::Source, AbiEventAccess::Read, 0)),
        ),
    ));
    let mismatched_order = mismatched_header_order();
    let mixed_operations = select_canonical_fault([copy_source_unmapped(), vector_source_a()]);

    // Then: each malformed class is rejected.
    assert_eq!(
        unknown_status,
        Err(AbiDecodeError::UnknownCompletionStatus { value: 9 })
    );
    assert_eq!(
        unknown_kind,
        Err(AbiDecodeError::UnknownEventKind { value: 99 })
    );
    assert_eq!(noncanonical, Err(AbiEncodeError::NonCanonicalEventDetails));
    assert_eq!(
        mismatched_order,
        Err(AbiFaultSelectionError::FaultDoesNotMatchOrder)
    );
    assert_eq!(
        mixed_operations,
        Err(AbiFaultSelectionError::MixedOperations)
    );
    let completion_flags_error = decode_completion(&completion_flags);
    let completion_reserved_error = decode_completion(&completion_reserved);
    let event_flags_error = decode_event(&event_flags);
    let event_reserved_error = decode_event(&event_reserved);
    let short_completion_error = decode_completion(&completion_reserved[..31]);
    let short_event = event_golden((1, 0x2000, 0x20, 0, 3, 1, 0));
    let short_event_error = decode_event(&short_event[..63]);
    assert!(completion_flags_error.is_err());
    assert!(completion_reserved_error.is_err());
    assert!(event_flags_error.is_err());
    assert!(event_reserved_error.is_err());
    assert_eq!(
        decode_event(&event_stage),
        Err(AbiDecodeError::UnknownEventStage { value: 99 })
    );
    assert_eq!(
        decode_event(&event_access),
        Err(AbiDecodeError::UnknownEventAccess { value: 99 })
    );
    assert!(short_completion_error.is_err());
    assert!(short_event_error.is_err());
    for malformed in noncanonical_memory_events()
        .into_iter()
        .chain(noncanonical_descriptor_events())
    {
        assert_eq!(
            encode_event(malformed),
            Err(AbiEncodeError::NonCanonicalEventDetails)
        );
    }
    println!("errors: {unknown_status:?}; {unknown_kind:?}; {noncanonical:?}");
    println!("ordering errors: {mismatched_order:?}; {mixed_operations:?}");
    println!(
        "malformed: {completion_flags_error:?}; {completion_reserved_error:?}; \
         {event_flags_error:?}; {event_reserved_error:?}; \
         {short_completion_error:?}; {short_event_error:?}"
    );
}

fn mismatched_header_order() -> Result<AbiFaultCandidate, AbiFaultSelectionError> {
    AbiFaultCandidate::try_new(
        AbiFault::invalid_size(DmaAddress::new(0x1000), 63),
        AbiFaultOrder::Descriptor(DescriptorViolationKind::Header(
            DescriptorHeaderSchema::VERSION_IDENTITY
                .at_byte(DescriptorHeaderSchema::VERSION_OFFSET),
        )),
    )
}
