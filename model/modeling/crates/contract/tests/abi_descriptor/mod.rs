//! ABI-v1 command descriptor golden vectors and malformed-input checks.

/// Malformed descriptor and boundary-error assertions.
pub mod errors;
/// Generated semantic descriptor-value assertions.
pub mod schema_values;
/// Typed descriptor fixtures and byte mutation helpers.
pub mod support;
/// Independent literal ABI-v1 golden byte arrays.
pub mod vectors;
/// Hardware-visible descriptor violation assertions.
pub mod violations;

use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, AbiCommandHeader, AbiCommandPayloadValues, BroadcastDestinationEntry,
    DESCRIPTOR_SIZE, DescriptorDecodeError, DescriptorViolationKind, DmaAddress,
    INDIRECT_ENTRY_ALIGNMENT,
};

#[test]
fn every_v1_descriptor_matches_golden_bytes() {
    // Given: four commands with non-palindromic IDs, nonzero absolute DMA addresses, and literal vectors.
    let commands = support::commands();
    let expected = [
        vectors::COPY,
        vectors::STRIDED,
        vectors::BROADCAST,
        vectors::VECTOR_ADD,
    ];
    let entry = BroadcastDestinationEntry::from_address(DmaAddress::new(0x0123_4567_89ab_cdef));

    // When: commands and an indirect destination cross both codec directions.
    let encoded = commands.map(support::encode);
    for (command, bytes) in commands.iter().zip(&expected) {
        assert_eq!(AbiCommand::decode(bytes), Ok(*command));
    }
    let mut entry_bytes = [0_u8; 8];
    let entry_result = entry.encode(&mut entry_bytes);
    let decoded_entry = BroadcastDestinationEntry::decode(&vectors::DESTINATION_ENTRY);

    // Then: byte offsets, LE order, indirect extent, round trips, are exact.
    assert_eq!(encoded, expected.map(Ok));
    assert_eq!(entry_result, Ok(()));
    assert_eq!(entry_bytes, vectors::DESTINATION_ENTRY);
    assert_eq!(decoded_entry, Ok(entry));
    let [_, _, broadcast, _] = commands;
    assert_eq!(
        match broadcast.payload() {
            AbiCommandPayloadValues::BroadcastCopy(value) => {
                u64::from(value.destination_count.value()) * INDIRECT_ENTRY_ALIGNMENT
            }
            AbiCommandPayloadValues::Copy(_)
            | AbiCommandPayloadValues::StridedCopy(_)
            | AbiCommandPayloadValues::VectorAdd(_) => 0,
        },
        24
    );
    let [copy_bytes, _, broadcast_bytes, _] = expected;
    println!("copy={copy_bytes:02x?} broadcast={broadcast_bytes:02x?} entry={entry_bytes:02x?}");
}

#[test]
fn rejects_wrong_version_size_opcode_flags_reserved_count_and_alignment() {
    // Given: canonical descriptors plus independent malformed header, payload, and slice cases.
    let mut malformed = vectors::COPY;
    assert!(support::replace_u16(&mut malformed, 0, 2));
    assert!(support::replace_u16(&mut malformed, 6, 63));
    assert!(support::replace_u16(&mut malformed, 2, 9));
    assert!(support::replace_u16(&mut malformed, 4, 1));

    // When: untrusted bytes and invalid host values cross descriptor boundaries.
    let first_error = AbiCommand::decode(&malformed);
    assert!(support::replace_u16(&mut malformed, 0, ABI_VERSION));
    let size_error = AbiCommand::decode(&malformed);
    assert!(support::replace_u16(&mut malformed, 6, DESCRIPTOR_SIZE));
    let opcode_error = AbiCommand::decode(&malformed);
    assert!(support::replace_u16(&mut malformed, 2, 1));
    let flags_error = AbiCommand::decode(&malformed);

    // Then: validation is ordered and every malformed class has one exact typed failure.
    assert!(matches!(
        first_error,
        Err(DescriptorDecodeError::Violation(violation))
            if violation.kind() == DescriptorViolationKind::Header(
                AbiCommandHeader::VERSION_IDENTITY
                    .at_byte(AbiCommandHeader::VERSION_OFFSET)
            )
                && violation.observed() == 2
    ));
    assert!(matches!(
        size_error,
        Err(DescriptorDecodeError::Violation(violation))
            if violation.kind() == DescriptorViolationKind::Header(
                AbiCommandHeader::SIZE_IDENTITY
                    .at_byte(AbiCommandHeader::SIZE_OFFSET)
            )
                && violation.observed() == 63
    ));
    assert!(matches!(
        opcode_error,
        Err(DescriptorDecodeError::Violation(violation))
            if violation.kind() == DescriptorViolationKind::Header(
                AbiCommandHeader::OPCODE_IDENTITY
                    .at_byte(AbiCommandHeader::OPCODE_OFFSET)
            )
                && violation.observed() == 9
    ));
    assert!(matches!(
        flags_error,
        Err(DescriptorDecodeError::Violation(violation))
            if violation.kind() == DescriptorViolationKind::Header(
                AbiCommandHeader::FLAGS_IDENTITY
                    .at_byte(AbiCommandHeader::FLAGS_OFFSET)
            )
                && violation.observed() == 1
    ));

    errors::assert_reserved_and_scalar_errors();
    errors::assert_count_alignment_and_slice_errors();
    println!(
        "ordered errors: version={first_error:?} size={size_error:?} opcode={opcode_error:?} flags={flags_error:?}"
    );
}
