use super::{support, vectors};
use pdma_model_contract::abi::{
    AbiCommand, AbiDecodeError, AbiEncodeError, AbiId, BroadcastDestinationEntry,
    DescriptorDecodeError, DescriptorSchemaPayloadBroadcastCopyValues, DescriptorValueError,
    INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS, MAX_STRIDE, MAX_STRIDED_ROWS,
    MAX_TRANSFER_BYTES,
};

fn assert_decode_error(bytes: &[u8], expected: DescriptorDecodeError) {
    assert_eq!(AbiCommand::decode(bytes), Err(expected));
}

/// Checks every operation-reserved word and bounded scalar.
///
/// # Panics
/// Panics when a malformed fixture is not rejected with its exact typed error.
pub fn assert_reserved_and_scalar_errors() {
    for (base, offsets) in [
        (vectors::COPY, &[40_usize, 48, 56][..]),
        (vectors::BROADCAST, &[48_usize, 56][..]),
        (vectors::VECTOR_ADD, &[48_usize, 56][..]),
    ] {
        for &offset in offsets {
            let mut bytes = base;
            assert!(support::replace_u64(
                &mut bytes,
                offset,
                0x0102_0304_0506_0708
            ));
            let offset = u16::try_from(offset);
            assert!(offset.is_ok());
            let Ok(offset) = offset else {
                return;
            };
            assert_decode_error(
                &bytes,
                DescriptorDecodeError::InvalidReserved {
                    offset,
                    observed: 0x0102_0304_0506_0708,
                },
            );
        }
    }

    for (base, offset, maximum) in [
        (vectors::COPY, 32, MAX_TRANSFER_BYTES),
        (vectors::STRIDED, 32, MAX_STRIDED_ROWS),
        (vectors::STRIDED, 40, MAX_TRANSFER_BYTES),
        (vectors::STRIDED, 48, MAX_STRIDE),
        (vectors::STRIDED, 56, MAX_STRIDE),
        (vectors::BROADCAST, 32, MAX_TRANSFER_BYTES),
        (vectors::VECTOR_ADD, 40, MAX_TRANSFER_BYTES),
    ] {
        let mut bytes = base;
        let observed = u64::from(maximum) + 1;
        assert!(support::replace_u64(&mut bytes, offset, observed));
        let offset = u16::try_from(offset);
        assert!(offset.is_ok());
        let Ok(offset) = offset else {
            return;
        };
        assert_decode_error(
            &bytes,
            DescriptorDecodeError::ValueOutOfRange {
                offset,
                observed,
                maximum,
            },
        );
    }

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
    assert_decode_error(
        &reserved_and_scalar,
        DescriptorDecodeError::InvalidReserved {
            offset: 40,
            observed: 0x0102_0304_0506_0708,
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
    assert_decode_error(
        &zero_count_and_misaligned,
        DescriptorDecodeError::InvalidIndirectCount {
            observed: 0,
            maximum: MAX_INDIRECT_DESTINATIONS,
        },
    );
    let mut zero_count = vectors::BROADCAST;
    assert!(support::replace_u64(&mut zero_count, 40, 0));
    assert_decode_error(
        &zero_count,
        DescriptorDecodeError::InvalidIndirectCount {
            observed: 0,
            maximum: MAX_INDIRECT_DESTINATIONS,
        },
    );
    let mut large_count = vectors::BROADCAST;
    assert!(support::replace_u64(
        &mut large_count,
        40,
        u64::from(u32::MAX) + 1
    ));
    assert_decode_error(
        &large_count,
        DescriptorDecodeError::InvalidIndirectCount {
            observed: u64::from(u32::MAX) + 1,
            maximum: MAX_INDIRECT_DESTINATIONS,
        },
    );
    let mut misaligned = vectors::BROADCAST;
    assert!(support::replace_u64(&mut misaligned, 24, 0x1003));
    assert_decode_error(
        &misaligned,
        DescriptorDecodeError::InvalidIndirectAlignment {
            address: 0x1003,
            alignment: INDIRECT_ENTRY_ALIGNMENT,
        },
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
