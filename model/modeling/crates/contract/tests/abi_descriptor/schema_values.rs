use super::{support, vectors};
use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE, DescriptorDecodeError,
    DescriptorHeaderSchemaFlagsValues, DescriptorHeaderSchemaOpcode, DescriptorHeaderSchemaValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadValues, DescriptorSchemaRaw,
    DescriptorSchemaValues, VECTOR_ADD_OPCODE,
};

#[test]
fn generated_descriptor_uses_semantic_header_and_payload_fields() {
    let values = DescriptorSchemaValues::from_bytes(&vectors::COPY);
    assert!(values.is_ok());
    let Ok(values) = values else {
        return;
    };
    assert_eq!(
        values,
        DescriptorSchemaValues {
            header: DescriptorHeaderSchemaValues {
                version: 1,
                opcode: DescriptorHeaderSchemaOpcode::Copy,
                flags: DescriptorHeaderSchemaFlagsValues::ZERO,
                command_id: 0x0123_4567_89ab_cdef,
            },
            payload: DescriptorSchemaPayloadValues::Copy(DescriptorSchemaPayloadCopyValues {
                source_dma_address: 0x1020_3040_5060_7080,
                destination_dma_address: 0x90a0_b0c0_d0e0_f000,
                length: 0x1122_3344,
            }),
        }
    );
    assert_eq!(values.to_bytes(), Ok(vectors::COPY));
}

#[test]
fn generated_descriptor_raw_and_abi_command_share_concrete_try_from_codecs() {
    // Given: one frozen descriptor raw owner.
    let raw = DescriptorSchemaRaw::from(vectors::COPY);

    // When: schema values and the validated ABI wrapper use concrete conversions.
    let values = DescriptorSchemaValues::try_from(raw);
    let command = AbiCommand::try_from(raw);

    // Then: both layers preserve the exact canonical ABI bytes.
    assert_eq!(
        values
            .and_then(DescriptorSchemaRaw::try_from)
            .map(DescriptorSchemaRaw::into_bytes),
        Ok(vectors::COPY)
    );
    let Ok(command) = command else {
        return;
    };
    assert_eq!(
        DescriptorSchemaRaw::try_from(command).map(DescriptorSchemaRaw::into_bytes),
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
    assert_eq!(
        AbiCommand::try_from(DescriptorSchemaRaw::from(bytes)),
        Err(DescriptorDecodeError::InvalidVersion {
            observed: ABI_VERSION + 1,
            expected: ABI_VERSION,
        })
    );
    assert!(support::replace_u16(&mut bytes, 0, ABI_VERSION));
    assert_eq!(
        AbiCommand::try_from(DescriptorSchemaRaw::from(bytes)),
        Err(DescriptorDecodeError::InvalidSize {
            observed: DESCRIPTOR_SIZE - 1,
            expected: DESCRIPTOR_SIZE,
        })
    );
    assert!(support::replace_u16(&mut bytes, 6, DESCRIPTOR_SIZE));
    assert_eq!(
        AbiCommand::try_from(DescriptorSchemaRaw::from(bytes)),
        Err(DescriptorDecodeError::InvalidOpcode {
            observed: VECTOR_ADD_OPCODE + 1,
            maximum: VECTOR_ADD_OPCODE,
        })
    );
    assert!(support::replace_u16(&mut bytes, 2, 1));
    assert_eq!(
        AbiCommand::try_from(DescriptorSchemaRaw::from(bytes)),
        Err(DescriptorDecodeError::InvalidFlags {
            observed: 1,
            allowed: DESCRIPTOR_FLAGS_MASK,
        })
    );
    assert!(support::replace_u16(&mut bytes, 4, DESCRIPTOR_FLAGS_MASK));
    assert_eq!(
        AbiCommand::try_from(DescriptorSchemaRaw::from(bytes)),
        Err(DescriptorDecodeError::InvalidReserved {
            offset: 40,
            observed: 1,
        })
    );
}
