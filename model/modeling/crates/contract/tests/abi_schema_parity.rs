//! ABI record-schema migration parity tests.

#[path = "abi_schema_parity/support.rs"]
pub(crate) mod support;

use pdma_model_contract::abi::{AbiCommand, AbiDecodeError};
use pdma_model_contract::abi::{
    BroadcastDestinationEntry, CompletionSchema, CompletionSchemaView, DescriptorSchema,
    DescriptorSchemaView, EventSchema, EventSchemaView, IndirectDestinationEntrySchema,
    IndirectDestinationEntrySchemaView,
};
use pdma_model_contract::bridge::{
    decode_completion, decode_event, encode_completion, encode_event,
};
use pdma_model_contract::semantic::{CommandId, CompletionRecord, CompletionStatus, QueueId};
use support::{
    COMPLETION_BYTES, COMPLETION_FIELDS, COPY, DESCRIPTOR_FIELDS, EVENT_BYTES, EVENT_FIELDS,
    INDIRECT_BYTES, INDIRECT_FIELDS, SchemaCodecError, assert_schema_parity,
};

#[test]
fn canonical_schemas_match_every_frozen_record_layout() {
    // Given: literal ABI-v1 layouts spanning every canonical DMA-visible record.
    let schemas = [
        (DescriptorSchema::SPEC, &DESCRIPTOR_FIELDS[..]),
        (CompletionSchema::SPEC, &COMPLETION_FIELDS[..]),
        (EventSchema::SPEC, &EVENT_FIELDS[..]),
        (IndirectDestinationEntrySchema::SPEC, &INDIRECT_FIELDS[..]),
    ];

    // When: legacy metadata is paired with generated reusable-schema metadata.
    for (schema, expected) in schemas {
        assert_schema_parity(schema, expected);
        assert_eq!(schema.validate(), Ok(()));
    }

    // Then: all generated views expose the same little-endian scalar boundaries.
    assert_eq!(
        DescriptorSchemaView::new(&COPY).unwrap().command_id(),
        Ok(0x0123_4567_89ab_cdef)
    );
    assert_eq!(
        CompletionSchemaView::new(&COMPLETION_BYTES)
            .unwrap()
            .origin_queue_id(),
        Ok(0x1122)
    );
    assert_eq!(
        EventSchemaView::new(&EVENT_BYTES).unwrap().fault_locator(),
        Ok(0x2000)
    );
    assert_eq!(
        IndirectDestinationEntrySchemaView::new(&INDIRECT_BYTES)
            .unwrap()
            .destination_iova(),
        Ok(0x0123_4567_89ab_cdef)
    );
}

#[test]
fn public_codecs_when_given_canonical_bytes_round_trip_without_drift() {
    // Given: frozen descriptor, completion, event, and indirect-entry bytes.
    let command = AbiCommand::decode(&COPY).unwrap();
    let completion = CompletionRecord::new(
        QueueId::new(0x1122),
        CommandId::new(0x3344),
        CompletionStatus::Success,
    );
    let event = decode_event(&EVENT_BYTES).unwrap();
    let entry = BroadcastDestinationEntry::decode(&INDIRECT_BYTES).unwrap();

    // When: callers use only the current public ABI codecs.
    let mut descriptor_output = [0_u8; 64];
    command.encode(&mut descriptor_output).unwrap();
    let encoded_completion = encode_completion(completion).unwrap();
    let encoded_event = encode_event(event).unwrap();
    let mut entry_output = [0_u8; 8];
    entry.encode(&mut entry_output).unwrap();

    // Then: the frozen bytes remain exactly canonical through the migration seam.
    assert_eq!(descriptor_output, COPY);
    assert_eq!(decode_completion(&COMPLETION_BYTES), Ok(completion));
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
    let completion_error = decode_completion(&completion);
    let event_error = decode_event(&event);

    // Then: public precedence remains ABI-first while generated views retain exact short errors.
    assert_eq!(
        descriptor_error,
        Err(
            pdma_model_contract::abi::DescriptorDecodeError::InvalidVersion {
                observed: 2,
                expected: 1
            }
        )
    );
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
        decode_completion(&COMPLETION_BYTES[..31]),
        Err(AbiDecodeError::BufferTooShort {
            required: 32,
            actual: 31
        })
    );
    assert_eq!(
        decode_event(&EVENT_BYTES[..63]),
        Err(AbiDecodeError::BufferTooShort {
            required: 64,
            actual: 63
        })
    );
    assert_eq!(
        DescriptorSchemaView::new(&COPY[..63]),
        Err(SchemaCodecError::BufferTooShort {
            required: 64,
            actual: 63
        })
    );
    assert_eq!(
        CompletionSchemaView::new(&COMPLETION_BYTES[..31]),
        Err(SchemaCodecError::BufferTooShort {
            required: 32,
            actual: 31
        })
    );
    assert_eq!(
        EventSchemaView::new(&EVENT_BYTES[..63]),
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
