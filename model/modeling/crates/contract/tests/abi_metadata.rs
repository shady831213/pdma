//! ABI-v1 numeric registry and checked boundary conformance.

use pdma_model_contract::BufferId;
use pdma_model_contract::abi::{
    ABI_VERSION, AbiAddressError, AbiControlError, AbiDecodeError, AbiEncodeError, AbiId,
    AbiV1Features, BROADCAST_COPY_OPCODE, COMPLETION, COMPLETION_ALIGNMENT, COMPLETION_FLAGS_MASK,
    COMPLETION_SIZE, COPY_OPCODE, DESCRIPTOR, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_FLAGS_MASK,
    DESCRIPTOR_SIZE, DESCRIPTOR_V1_FEATURE, DmaAddress, DmaRegion, EVENT, EVENT_ACCESSES,
    EVENT_ALIGNMENT, EVENT_FLAGS_MASK, EVENT_KINDS, EVENT_SIZE, EVENT_STAGES, EVENT_V1_FEATURE,
    FEATURES, FieldSpec, INDIRECT_BROADCAST_FEATURE, INDIRECT_ENTRY_ALIGNMENT, IOVA64_FEATURE,
    KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS, MAX_RING_DEPTH, MAX_STRIDE, MAX_STRIDED_ROWS,
    MAX_TRANSFER_BYTES, OPCODES, OpcodeSpec, STRIDED_COPY_FEATURE, STRIDED_COPY_OPCODE,
    SUCCESS_STATUS, VECTOR_ADD_FEATURE, VECTOR_ADD_OPCODE, copy_u64_le, read_u16_le, read_u64_le,
    validate_abi_version, write_u16_le, write_u64_le,
};

fn assert_fields_cover_record(fields: &[FieldSpec], record_size: u16) {
    for (index, field) in fields.iter().enumerate() {
        assert!(field.width() > 0);
        assert!(field.end() <= record_size);
        for other in fields.iter().skip(index + 1) {
            assert!(
                field.end() <= other.offset() || other.end() <= field.offset(),
                "{} overlaps {}",
                field.name(),
                other.name()
            );
        }
    }
}

fn assert_field_ranges(fields: &[FieldSpec], expected: &[(u16, u16)]) {
    assert_eq!(fields.len(), expected.len());
    for (field, &(start, end)) in fields.iter().zip(expected) {
        assert_eq!((field.offset(), field.end()), (start, end));
    }
}

fn assert_record_registry() {
    assert_eq!([DESCRIPTOR_SIZE, COMPLETION_SIZE, EVENT_SIZE], [64, 32, 64]);
    assert_eq!(
        [
            DESCRIPTOR_ALIGNMENT,
            COMPLETION_ALIGNMENT,
            EVENT_ALIGNMENT,
            INDIRECT_ENTRY_ALIGNMENT
        ],
        [64, 32, 64, 8]
    );
    assert_field_ranges(
        DESCRIPTOR.fields(),
        &[
            (0, 2),
            (2, 4),
            (4, 6),
            (6, 8),
            (8, 16),
            (16, 24),
            (24, 32),
            (32, 40),
            (40, 48),
            (48, 56),
            (56, 64),
        ],
    );
    assert_field_ranges(
        COMPLETION.fields(),
        &[(0, 2), (2, 4), (4, 6), (6, 8), (8, 16), (16, 24), (24, 32)],
    );
    assert_field_ranges(
        EVENT.fields(),
        &[
            (0, 2),
            (2, 4),
            (4, 6),
            (6, 8),
            (8, 16),
            (16, 24),
            (24, 32),
            (32, 40),
            (40, 48),
            (48, 50),
            (50, 52),
            (52, 56),
            (56, 64),
        ],
    );
    assert_fields_cover_record(DESCRIPTOR.fields(), 64);
    assert_fields_cover_record(COMPLETION.fields(), 32);
    assert_fields_cover_record(EVENT.fields(), 64);
    assert_eq!(
        [DESCRIPTOR.size(), COMPLETION.size(), EVENT.size(),],
        [64, 32, 64]
    );
    assert_eq!(
        [
            DESCRIPTOR.alignment(),
            COMPLETION.alignment(),
            EVENT.alignment(),
        ],
        [64, 32, 64]
    );
}

#[test]
fn abi_v1_registry_is_exact() {
    // Given: independent literal expectations for the public ABI v1 registry.
    let expected_features = [1_u64, 2, 4, 8, 16, 32, 64, 128];
    let sentinel = 0x0123_4567_89ab_cdef_u64;
    let mut bytes = [0_u8; 10];

    // When: callers consume canonical metadata and the checked LE helpers.
    write_u16_le(&mut bytes, 0..2, 0x2301).expect("two-byte range is valid");
    write_u64_le(&mut bytes, 2..10, sentinel).expect("eight-byte range is valid");
    let decoded_version = read_u16_le(&bytes, 0..2).expect("two-byte range is valid");
    let decoded_sentinel = read_u64_le(&bytes, 2..10).expect("eight-byte range is valid");

    // Then: numeric assignments, bounds, and byte order exactly match the contract.
    assert_eq!(ABI_VERSION, 1);
    assert_eq!(IOVA64_FEATURE, 1);
    assert_eq!(DESCRIPTOR_V1_FEATURE, 2);
    assert_eq!(pdma_model_contract::abi::COMPLETION_V1_FEATURE, 4);
    assert_eq!(EVENT_V1_FEATURE, 8);
    assert_eq!(pdma_model_contract::abi::COPY_FEATURE, 16);
    assert_eq!(STRIDED_COPY_FEATURE, 32);
    assert_eq!(INDIRECT_BROADCAST_FEATURE, 64);
    assert_eq!(VECTOR_ADD_FEATURE, 128);
    assert_eq!(KNOWN_FEATURE_MASK, 0xff);
    assert_eq!(FEATURES.map(|(_, bit)| bit), expected_features);
    assert_eq!(
        FEATURES.map(|(name, _)| name),
        [
            "IOVA64",
            "DESCRIPTOR_V1",
            "COMPLETION_V1",
            "EVENT_V1",
            "COPY",
            "STRIDED_COPY",
            "INDIRECT_BROADCAST",
            "VECTOR_ADD",
        ]
    );
    assert_eq!(
        AbiV1Features::try_from(KNOWN_FEATURE_MASK)
            .expect("known mask is valid")
            .bits(),
        0xff
    );
    assert_eq!(COPY_OPCODE, 1);
    assert_eq!(STRIDED_COPY_OPCODE, 2);
    assert_eq!(BROADCAST_COPY_OPCODE, 3);
    assert_eq!(VECTOR_ADD_OPCODE, 4);
    assert_eq!(OPCODES.map(OpcodeSpec::value), [1, 2, 3, 4]);
    assert_eq!(
        EVENT_KINDS.map(|entry| entry.value),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(EVENT_STAGES.map(|entry| entry.value), [1, 2, 3, 4]);
    assert_eq!(EVENT_ACCESSES.map(|entry| entry.value), [0, 1, 2]);
    assert_eq!(DESCRIPTOR_FLAGS_MASK, 0);
    assert_eq!(COMPLETION_FLAGS_MASK, 0);
    assert_eq!(EVENT_FLAGS_MASK, 0);
    assert_eq!(SUCCESS_STATUS, 0);
    assert_eq!(
        [
            MAX_RING_DEPTH,
            MAX_INDIRECT_DESTINATIONS,
            MAX_TRANSFER_BYTES,
            MAX_STRIDED_ROWS,
            MAX_STRIDE,
        ],
        [u32::MAX; 5]
    );
    assert_record_registry();
    assert_eq!(
        bytes,
        [0x01, 0x23, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]
    );
    assert_eq!(decoded_version, 0x2301);
    assert_eq!(decoded_sentinel, sentinel);
    assert_eq!(
        copy_u64_le(sentinel),
        [0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]
    );
    println!(
        "ABI v{ABI_VERSION}: features={expected_features:?} opcodes={:?} event_kinds={:?} bytes={bytes:02x?}",
        OPCODES.map(OpcodeSpec::value),
        EVENT_KINDS.map(|entry| entry.value),
    );
}

#[test]
fn rejects_unknown_features_and_unrepresentable_ids() {
    // Given: one unknown feature and values that cannot cross fixed-width boundaries.
    let unknown_features = 1_u64 << 8;
    let oversized_id = u128::from(u64::MAX) + 1;
    let short = [0_u8; 7];
    let mut short_output = [0_u8; 7];
    let invalid_range = read_u16_le(&short, 2..5);
    let address_error = DmaAddress::try_from(oversized_id);
    let region_error = DmaRegion::try_new(BufferId::new(3), DmaAddress::new(u64::MAX), 1);
    let version_error = validate_abi_version(2);

    // When: each untrusted value crosses its typed ABI boundary.
    let feature_error = AbiV1Features::try_from(unknown_features);
    let id_error = AbiId::try_from(oversized_id);
    let read_error = read_u64_le(&short, 0..8);
    let write_error = write_u64_le(&mut short_output, 0..8, 7);

    // Then: every malformed input is rejected without indexing or truncation.
    assert_eq!(
        feature_error,
        Err(AbiDecodeError::UnknownFeatureBits { bits: 1_u64 << 8 })
    );
    assert_eq!(
        id_error,
        Err(AbiEncodeError::UnrepresentableId {
            value: oversized_id
        })
    );
    assert_eq!(
        read_error,
        Err(AbiDecodeError::BufferTooShort {
            required: 8,
            actual: 7
        })
    );
    assert_eq!(
        write_error,
        Err(AbiEncodeError::BufferTooShort {
            required: 8,
            actual: 7
        })
    );
    assert_eq!(
        invalid_range,
        Err(AbiDecodeError::InvalidFieldRange {
            start: 2,
            end: 5,
            width: 2,
        })
    );
    assert_eq!(
        address_error,
        Err(AbiAddressError::UnrepresentableAddress {
            value: oversized_id,
        })
    );
    assert_eq!(
        region_error,
        Err(AbiAddressError::RangeOverflow {
            base: u64::MAX,
            length: 1,
        })
    );
    assert_eq!(
        version_error,
        Err(AbiControlError::UnsupportedVersion { requested: 2 })
    );
    println!(
        "typed failures: feature={feature_error:?} id={id_error:?} short_read={read_error:?} address={address_error:?}"
    );
}
