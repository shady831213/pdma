//! ABI record-schema migration parity tests.

#[path = "abi_schema_parity/support.rs"]
pub(crate) mod support;

use pdma_model_contract::abi::{
    AbiCommand, AbiCommandHeader, AbiCompletion, AbiCompletionRaw, AbiCompletionView,
    AbiDecodeError, AbiEvent, AbiEventRaw, AbiEventView, DescriptorDecodeError,
    DescriptorViolationKind,
};
use pdma_model_contract::abi::{
    AbiCommandHeaderOpcode, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCommandView,
    BroadcastDestinationEntry, IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaView,
};
use support::{
    COMPLETION_BYTES, COMPLETION_FIELDS, COPY, DESCRIPTOR_FIELDS, EVENT_BYTES, EVENT_FIELDS,
    INDIRECT_BYTES, INDIRECT_FIELDS, SchemaCodecError, assert_schema_parity,
};

#[test]
fn completion_and_event_are_the_generated_record_values() {
    // Given: independently frozen completion and event bytes.
    let completion_raw = AbiCompletionRaw::new(COMPLETION_BYTES);
    let event_raw = AbiEventRaw::new(EVENT_BYTES);

    // When: the macro-generated raw owners decode into the public ABI values.
    let completion = AbiCompletion::try_from(completion_raw);
    let event = AbiEvent::try_from(event_raw);

    // Then: the public values expose the complete wire fields without a wrapper.
    assert!(matches!(completion, Ok(value) if value.origin_queue_id == 0x1122));
    assert!(matches!(event, Ok(value) if value.fault_locator == 0x2000));
}

#[test]
fn canonical_schemas_match_every_frozen_record_layout() {
    // Given: literal ABI-v1 layouts spanning every canonical DMA-visible record.
    let schemas = [
        (AbiCommand::SPEC, &DESCRIPTOR_FIELDS[..]),
        (AbiCompletion::SPEC, &COMPLETION_FIELDS[..]),
        (AbiEvent::SPEC, &EVENT_FIELDS[..]),
        (IndirectDestinationEntrySchema::SPEC, &INDIRECT_FIELDS[..]),
    ];

    // When: legacy metadata is paired with generated reusable-schema metadata.
    for (schema, expected) in schemas {
        assert_schema_parity(schema, expected);
        assert_eq!(schema.validate(), Ok(()));
    }

    // Then: all generated views expose the same little-endian scalar boundaries.
    assert_eq!(
        AbiCommandView::new(&COPY)
            .unwrap()
            .header()
            .unwrap()
            .command_id(),
        Ok(0x0123_4567_89ab_cdef)
    );
    assert_eq!(
        AbiCompletionView::new(&COMPLETION_BYTES)
            .unwrap()
            .origin_queue_id(),
        Ok(0x1122)
    );
    assert_eq!(
        AbiEventView::new(&EVENT_BYTES).unwrap().fault_locator(),
        Ok(0x2000)
    );
    assert_eq!(
        IndirectDestinationEntrySchemaView::new(&INDIRECT_BYTES)
            .unwrap()
            .destination_dma_address(),
        Ok(0x0123_4567_89ab_cdef)
    );
}

#[test]
fn descriptor_owned_values_expose_the_complete_frozen_wire_record() {
    // Given: the independent literal linear-copy descriptor bytes.
    // When: the generated owned record codec decodes them.
    let values = AbiCommand::from_bytes(&COPY);

    // Then: header, command identity, payload words, and exact re-encoding are all generated.
    assert!(values.is_ok());
    let Ok(values) = values else {
        return;
    };
    assert_eq!(values.header.version, 1);
    assert_eq!(values.header.opcode, AbiCommandHeaderOpcode::Copy);
    assert_eq!(values.header.command_id, 0x0123_4567_89ab_cdef);
    assert_eq!(
        values.payload,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x1020_3040_5060_7080,
            destination_dma_address: 0x90a0_b0c0_d0e0_f000,
            length: 0x1122_3344.into(),
        })
    );
    assert_eq!(values.to_bytes(), Ok(COPY));
}

#[test]
fn record_raw_owners_are_the_public_typed_conversion_seam() {
    // Given: independently frozen completion and event bytes.
    let completion_raw = AbiCompletionRaw::new(COMPLETION_BYTES);
    let event_raw = AbiEventRaw::new(EVENT_BYTES);

    // When: the exact raw owners cross the public typed conversion seam in both directions.
    let completion = AbiCompletion::try_from(completion_raw);
    let event = AbiEvent::try_from(event_raw);

    // Then: validation succeeds and reverse conversion preserves every frozen byte.
    assert!(completion.is_ok());
    assert!(event.is_ok());
    let Ok(completion) = completion else {
        return;
    };
    let Ok(event) = event else {
        return;
    };
    assert_eq!(AbiCompletionRaw::try_from(completion), Ok(completion_raw));
    assert_eq!(AbiEventRaw::try_from(event), Ok(event_raw));
}

#[test]
fn record_raw_conversion_preserves_first_error_order() {
    // Given: raw records with multiple independently invalid fields.
    let mut completion = COMPLETION_BYTES;
    completion[6] = 1;
    completion[24] = 1;
    let mut event = EVENT_BYTES;
    event[48] = 99;
    event[50] = 98;
    event[52] = 1;

    // When: the raw owners cross the typed validation seam.
    let completion_error = AbiCompletion::decode(&completion);
    let event_error = AbiEvent::decode(&event);

    // Then: the earliest ABI-v1 violation is returned for each record.
    assert_eq!(
        completion_error,
        Err(AbiDecodeError::InvalidRecordField {
            field: "flags",
            value: 1
        })
    );
    assert_eq!(
        event_error,
        Err(AbiDecodeError::UnknownEventStage { value: 99 })
    );
}

#[test]
fn record_raw_conversion_rejects_noncanonical_event_details() {
    // Given: an unmapped-address event with a forbidden secondary detail.
    let mut event = EVENT_BYTES;
    event[40] = 1;

    // When: the raw event crosses the typed validation seam.
    let result = AbiEvent::decode(&event);

    // Then: semantic tuple canonicality is enforced after field validation.
    assert_eq!(result, Err(AbiDecodeError::NonCanonicalEventDetails));
}

#[test]
fn public_codecs_when_given_canonical_bytes_round_trip_without_drift() {
    // Given: frozen descriptor, completion, event, and indirect-entry bytes.
    let command = AbiCommand::decode(&COPY).unwrap();
    let completion = AbiCompletion::success(0x1122, 0x3344);
    let event = AbiEvent::decode(&EVENT_BYTES).unwrap();
    let entry = BroadcastDestinationEntry::decode(&INDIRECT_BYTES).unwrap();

    // When: callers use only the current public ABI codecs.
    let mut descriptor_output = [0_u8; 64];
    command.encode(&mut descriptor_output).unwrap();
    let mut encoded_completion = [0_u8; AbiCompletion::RECORD_SIZE];
    completion.encode(&mut encoded_completion).unwrap();
    let mut encoded_event = [0_u8; AbiEvent::RECORD_SIZE];
    event.encode(&mut encoded_event).unwrap();
    let mut entry_output = [0_u8; 8];
    entry.encode(&mut entry_output).unwrap();

    // Then: the frozen bytes remain exactly canonical through the migration seam.
    assert_eq!(descriptor_output, COPY);
    assert_eq!(AbiCompletion::decode(&COMPLETION_BYTES), Ok(completion));
    assert_eq!(encoded_completion, COMPLETION_BYTES);
    assert_eq!(encoded_event, EVENT_BYTES);
    assert_eq!(entry_output, INDIRECT_BYTES);
}

#[test]
fn public_error_order_when_malformed_input_crosses_old_and_new_boundaries() {
    // Given: short and conflicting fields in otherwise canonical ABI-v1 records.
    let mut descriptor = COPY;
    descriptor[0] = 2;
    descriptor[2] = 9;
    let mut completion = COMPLETION_BYTES;
    completion[6] = 1;
    let mut event = EVENT_BYTES;
    event[48] = 99;

    // When: existing public codecs and generated views parse the malformed bytes.
    let descriptor_error = AbiCommand::decode(&descriptor);
    let completion_error = AbiCompletion::decode(&completion);
    let event_error = AbiEvent::decode(&event);

    // Then: public precedence remains ABI-first while generated views retain exact short errors.
    assert!(matches!(
        descriptor_error,
        Err(DescriptorDecodeError::Violation(violation))
            if violation.kind() == DescriptorViolationKind::Header(
                AbiCommandHeader::VERSION_IDENTITY
                    .at_byte(AbiCommandHeader::VERSION_OFFSET)
            )
                && violation.observed() == 2
                && violation.expected() == 1
    ));
    assert_eq!(
        completion_error,
        Err(AbiDecodeError::InvalidRecordField {
            field: "flags",
            value: 1
        })
    );
    assert_eq!(
        event_error,
        Err(AbiDecodeError::UnknownEventStage { value: 99 })
    );
    assert_eq!(
        AbiCommand::decode(&COPY[..63]),
        Err(pdma_model_contract::abi::DescriptorDecodeError::Codec(
            AbiDecodeError::BufferTooShort {
                required: 64,
                actual: 63
            }
        ))
    );
    assert_eq!(
        BroadcastDestinationEntry::decode(&INDIRECT_BYTES[..7]),
        Err(pdma_model_contract::abi::DescriptorDecodeError::Codec(
            AbiDecodeError::BufferTooShort {
                required: 8,
                actual: 7
            }
        ))
    );
    assert_eq!(
        AbiCompletion::decode(&COMPLETION_BYTES[..31]),
        Err(AbiDecodeError::BufferTooShort {
            required: 32,
            actual: 31
        })
    );
    assert_eq!(
        AbiEvent::decode(&EVENT_BYTES[..63]),
        Err(AbiDecodeError::BufferTooShort {
            required: 64,
            actual: 63
        })
    );
    assert_eq!(
        AbiCommandView::new(&COPY[..63]),
        Err(SchemaCodecError::BufferTooShort {
            required: 64,
            actual: 63
        })
    );
    assert_eq!(
        AbiCompletionView::new(&COMPLETION_BYTES[..31]),
        Err(SchemaCodecError::BufferTooShort {
            required: 32,
            actual: 31
        })
    );
    assert_eq!(
        AbiEventView::new(&EVENT_BYTES[..63]),
        Err(SchemaCodecError::BufferTooShort {
            required: 64,
            actual: 63
        })
    );
    assert_eq!(
        IndirectDestinationEntrySchemaView::new(&INDIRECT_BYTES[..7]),
        Err(SchemaCodecError::BufferTooShort {
            required: 8,
            actual: 7
        })
    );
}
