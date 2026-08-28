use super::{support, vectors};
use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, AbiCommandHeader, AbiCommandHeaderFlagsValues, AbiCommandHeaderOpcode,
    AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues, AbiCommandPayloadValues,
    AbiCommandPayloadVectorAddValues, AbiCommandRaw, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorDecodeError, DescriptorViolationKind, VECTOR_ADD_OPCODE,
};

fn assert_violation(
    result: Result<AbiCommand, DescriptorDecodeError>,
    kind: DescriptorViolationKind,
    observed: u64,
) {
    assert!(matches!(result, Err(DescriptorDecodeError::Violation(_))));
    let Err(DescriptorDecodeError::Violation(violation)) = result else {
        return;
    };
    assert_eq!(violation.kind(), kind);
    assert_eq!(violation.observed(), observed);
}

#[test]
fn generated_descriptor_uses_semantic_header_and_payload_fields() {
    let values = AbiCommand::from_bytes(&vectors::COPY);
    assert!(values.is_ok());
    let Ok(values) = values else {
        return;
    };
    assert_eq!(
        values,
        AbiCommand {
            header: AbiCommandHeader {
                version: 1,
                opcode: AbiCommandHeaderOpcode::Copy,
                flags: AbiCommandHeaderFlagsValues::ZERO,
                command_id: 0x0123_4567_89ab_cdef,
            },
            payload: AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: 0x1020_3040_5060_7080,
                destination_dma_address: 0x90a0_b0c0_d0e0_f000,
                length: 0x1122_3344.into(),
            }),
        }
    );
    assert_eq!(values.to_bytes(), Ok(vectors::COPY));
}

#[test]
fn generated_descriptor_error_identifies_broadcast_destination_count() {
    // Given: a broadcast descriptor whose stored count exceeds its semantic width.
    let mut bytes = vectors::BROADCAST;
    let observed = u64::from(u32::MAX) + 1;
    assert!(support::replace_u64(&mut bytes, 40, observed));

    // When: the generated schema converts the exact raw descriptor.
    let decoded = AbiCommand::try_from(AbiCommandRaw::from(bytes));

    // Then: the error names the union leaf while retaining its wire offset.
    assert_eq!(
        decoded,
        Err(SchemaCodecError::NonZeroReserved {
            location: AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY.at_bit(352),
            observed,
        })
    );
}

#[test]
fn destination_count_identity_maps_count_error_at_its_generated_byte() {
    // Given: a broadcast count above its semantic width at the generated count member byte.
    let mut bytes = vectors::BROADCAST;
    let observed = u64::from(u32::MAX) + 1;
    assert!(support::replace_u64(&mut bytes, 40, observed));

    // When: the public descriptor boundary decodes the raw schema value.
    let decoded = AbiCommand::decode(&bytes);

    // Then: the count member maps to its descriptor-specific public error.
    assert_violation(
        decoded,
        DescriptorViolationKind::Scalar(
            AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(AbiCommand::WORD3_OFFSET),
        ),
        observed,
    );
}

#[test]
fn vector_length_at_broadcast_count_byte_remains_value_error() {
    // Given: vector length at byte 40, shared with the broadcast count's wire placement.
    let mut bytes = vectors::VECTOR_ADD;
    let observed = u64::from(u32::MAX) + 1;
    assert!(support::replace_u64(&mut bytes, 40, observed));

    // When: the public descriptor boundary decodes the raw schema value.
    let decoded = AbiCommand::decode(&bytes);

    // Then: the distinct generated member remains an ordinary scalar range error.
    assert_violation(
        decoded,
        DescriptorViolationKind::Scalar(
            AbiCommandPayloadVectorAddValues::LENGTH_IDENTITY.at_byte(AbiCommand::WORD3_OFFSET),
        ),
        observed,
    );
}

#[test]
fn generated_descriptor_raw_and_abi_command_share_concrete_try_from_codecs() {
    // Given: one frozen descriptor raw owner.
    let raw = AbiCommandRaw::from(vectors::COPY);

    // When: structural and validated conversions consume the same generated type.
    let values = AbiCommand::try_from(raw);
    let command = AbiCommand::decode(raw.as_bytes());

    // Then: both layers preserve the exact canonical ABI bytes.
    assert_eq!(
        values
            .and_then(AbiCommandRaw::try_from)
            .map(AbiCommandRaw::into_bytes),
        Ok(vectors::COPY)
    );
    let Ok(command) = command else {
        return;
    };
    assert_eq!(
        AbiCommandRaw::try_from(command).map(AbiCommandRaw::into_bytes),
        Ok(vectors::COPY)
    );
}

#[test]
fn generated_descriptor_raw_try_from_preserves_error_precedence() {
    // Given: one raw descriptor with every header field and one payload field malformed.
    let mut bytes = vectors::COPY;
    assert!(support::replace_u16(&mut bytes, 0, ABI_VERSION + 1));
    assert!(support::replace_u16(&mut bytes, 6, DESCRIPTOR_SIZE - 1));
    assert!(support::replace_u16(&mut bytes, 2, VECTOR_ADD_OPCODE + 1));
    assert!(support::replace_u16(&mut bytes, 4, 1));
    assert!(support::replace_u64(&mut bytes, 40, 1));

    // When/Then: direct raw conversion keeps the canonical validation order.
    assert_violation(
        AbiCommand::decode(&bytes),
        DescriptorViolationKind::Header(
            AbiCommandHeader::VERSION_IDENTITY.at_byte(AbiCommandHeader::VERSION_OFFSET),
        ),
        u64::from(ABI_VERSION + 1),
    );
    assert!(support::replace_u16(&mut bytes, 0, ABI_VERSION));
    assert_violation(
        AbiCommand::decode(&bytes),
        DescriptorViolationKind::Header(
            AbiCommandHeader::SIZE_IDENTITY.at_byte(AbiCommandHeader::SIZE_OFFSET),
        ),
        u64::from(DESCRIPTOR_SIZE - 1),
    );
    assert!(support::replace_u16(&mut bytes, 6, DESCRIPTOR_SIZE));
    assert_violation(
        AbiCommand::decode(&bytes),
        DescriptorViolationKind::Header(
            AbiCommandHeader::OPCODE_IDENTITY.at_byte(AbiCommandHeader::OPCODE_OFFSET),
        ),
        u64::from(VECTOR_ADD_OPCODE + 1),
    );
    assert!(support::replace_u16(&mut bytes, 2, 1));
    assert_violation(
        AbiCommand::decode(&bytes),
        DescriptorViolationKind::Header(
            AbiCommandHeader::FLAGS_IDENTITY.at_byte(AbiCommandHeader::FLAGS_OFFSET),
        ),
        1,
    );
    assert!(support::replace_u16(&mut bytes, 4, DESCRIPTOR_FLAGS_MASK));
    assert_violation(
        AbiCommand::decode(&bytes),
        DescriptorViolationKind::Reserved {
            opcode: AbiCommandHeaderOpcode::Copy,
            location: AbiCommand::PAYLOAD_IDENTITY.at_byte(AbiCommand::WORD3_OFFSET),
        },
        1,
    );
}
