//! ABI-v1 command descriptor golden vectors and malformed-input checks.

/// Malformed descriptor and boundary-error assertions.
#[path = "abi_descriptor/errors.rs"]
pub mod errors;
/// Typed descriptor fixtures and byte mutation helpers.
#[path = "abi_descriptor/support.rs"]
pub mod support;
/// Independent literal ABI-v1 golden byte arrays.
#[path = "abi_descriptor/vectors.rs"]
pub mod vectors;

use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, BroadcastDestinationEntry, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorDecodeError, DmaAddress, VECTOR_ADD_OPCODE,
};
use pdma_model_contract::semantic::{BufferId, CopyRequest, DmaRangeStart, StridedCopyRequest};

#[test]
fn every_v1_descriptor_matches_golden_bytes() {
    // Given: four commands with non-palindromic IDs, nonzero absolute IOVAs, and literal vectors.
    let commands = support::commands();
    assert!(commands.is_ok());
    let Ok(commands) = commands else {
        return;
    };
    let expected = [
        vectors::COPY,
        vectors::STRIDED,
        vectors::BROADCAST,
        vectors::VECTOR_ADD,
    ];
    let entry = BroadcastDestinationEntry::new(DmaAddress::new(0x0123_4567_89ab_cdef));
    let old_copy = CopyRequest::new(BufferId::new(1), BufferId::new(2), 5);
    let copy = CopyRequest::with_starts(
        DmaRangeStart::new(BufferId::new(1), 7),
        DmaRangeStart::new(BufferId::new(2), 11),
        5,
    );
    let old_strided = StridedCopyRequest::new(BufferId::new(3), BufferId::new(4), 2, 3, 5, 7);
    let strided = StridedCopyRequest::with_starts(
        DmaRangeStart::new(BufferId::new(3), 13),
        DmaRangeStart::new(BufferId::new(4), 17),
        2,
        3,
        5,
        7,
    );

    // When: commands and an indirect destination cross both codec directions.
    let encoded = commands.map(support::encode);
    for (command, bytes) in commands.iter().zip(&expected) {
        assert_eq!(AbiCommand::decode(bytes), Ok(*command));
    }
    let mut entry_bytes = [0_u8; 8];
    let entry_result = entry.encode(&mut entry_bytes);
    let decoded_entry = BroadcastDestinationEntry::decode(&vectors::DESTINATION_ENTRY);

    // Then: byte offsets, LE order, indirect extent, round trips, and semantic starts are exact.
    assert_eq!(encoded, expected.map(Ok));
    assert_eq!(entry_result, Ok(()));
    assert_eq!(entry_bytes, vectors::DESTINATION_ENTRY);
    assert_eq!(decoded_entry, Ok(entry));
    let [_, _, broadcast, _] = commands;
    assert_eq!(
        match broadcast {
            AbiCommand::BroadcastCopy(value) => value.destination_list_extent(),
            _ => 0,
        },
        24
    );
    assert_eq!(
        (old_copy.src_start().offset(), old_copy.dst_start().offset()),
        (0, 0)
    );
    assert_eq!(
        (copy.src_start().offset(), copy.dst_start().offset()),
        (7, 11)
    );
    assert_eq!(
        (
            old_strided.src_start().offset(),
            old_strided.dst_start().offset()
        ),
        (0, 0)
    );
    assert_eq!(
        (strided.src_start().offset(), strided.dst_start().offset()),
        (13, 17)
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
    assert_eq!(
        first_error,
        Err(DescriptorDecodeError::InvalidVersion {
            observed: 2,
            expected: 1
        })
    );
    assert_eq!(
        size_error,
        Err(DescriptorDecodeError::InvalidSize {
            observed: 63,
            expected: 64
        })
    );
    assert_eq!(
        opcode_error,
        Err(DescriptorDecodeError::InvalidOpcode {
            observed: 9,
            maximum: VECTOR_ADD_OPCODE,
        })
    );
    assert_eq!(
        flags_error,
        Err(DescriptorDecodeError::InvalidFlags {
            observed: 1,
            allowed: DESCRIPTOR_FLAGS_MASK,
        })
    );

    errors::assert_reserved_and_scalar_errors();
    errors::assert_count_alignment_and_slice_errors();
    println!(
        "ordered errors: version={first_error:?} size={size_error:?} opcode={opcode_error:?} flags={flags_error:?}"
    );
}
