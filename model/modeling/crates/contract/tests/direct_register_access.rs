//! Raw direct-register access decoding and error precedence.

use pdma_model_contract::abi::registers::{
    MmioAccessError, QueueRegisters, QueueRegistersRegisterId, RegisterLocation, decode_read,
    decode_write, locate, queue_offset,
};

#[test]
fn writes_decode_full_little_endian_scalars() {
    // Given
    let depth = [0xef, 0xcd, 0xab, 0x89];
    let base = [0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01];

    // When
    let decoded_depth = decode_write(72, &depth);
    let decoded_base = decode_write(64, &base);

    // Then
    assert_eq!(
        decoded_depth,
        Ok((locate(72).expect("SQ depth"), 0x89ab_cdef))
    );
    assert_eq!(
        decoded_base,
        Ok((locate(64).expect("SQ base"), 0x0123_4567_89ab_cdef))
    );
}

#[test]
fn unsupported_width_precedes_aperture_alignment_and_permissions() {
    for width in [0, 1, 2, 16, 256, u128::MAX] {
        // Given
        let offset = u32::MAX;

        // When
        let result = decode_read(offset, width);

        // Then
        assert_eq!(
            result,
            Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: width,
            })
        );
    }
}

#[test]
fn malformed_write_lengths_are_lossless() {
    for bytes in [
        &[][..],
        &[0; 1][..],
        &[0; 2][..],
        &[0; 16][..],
        &[0; 256][..],
    ] {
        // Given
        let offset = u32::MAX;

        // When
        let result = decode_write(offset, bytes);

        // Then
        assert_eq!(
            result,
            Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: u128::try_from(bytes.len()).expect("byte count"),
            })
        );
    }
}

#[test]
fn malformed_reads_follow_structural_precedence() {
    // Given
    let cases = [
        (
            u32::MAX - 3,
            8,
            MmioAccessError::UnmappedOffset {
                offset: u32::MAX - 3,
            },
        ),
        (
            65,
            8,
            MmioAccessError::Misaligned {
                offset: 65,
                width: 8,
            },
        ),
        (60, 4, MmioAccessError::ReservedOffset { offset: 60 }),
        (68, 4, MmioAccessError::ReservedOffset { offset: 68 }),
        (
            64,
            4,
            MmioAccessError::WrongWidth {
                offset: 64,
                expected: 8,
                actual: 4,
            },
        ),
        (
            72,
            8,
            MmioAccessError::WrongWidth {
                offset: 72,
                expected: 4,
                actual: 8,
            },
        ),
        (52, 4, MmioAccessError::ReadFromWriteOnly { offset: 52 }),
        (136, 8, MmioAccessError::ReadFromWriteOnly { offset: 136 }),
        (200, 4, MmioAccessError::ReadFromWriteOnly { offset: 200 }),
    ];

    // When / Then
    for (offset, width, expected) in cases {
        assert_eq!(decode_read(offset, width), Err(expected));
    }
}

#[test]
fn write_permissions_precede_reserved_bits() {
    // Given
    let cases = [
        (0, 4, MmioAccessError::WriteToReadOnly { offset: 0 }),
        (144, 8, MmioAccessError::WriteToReadOnly { offset: 144 }),
        (148, 4, MmioAccessError::ReservedOffset { offset: 148 }),
        (
            64,
            4,
            MmioAccessError::WrongWidth {
                offset: 64,
                expected: 8,
                actual: 4,
            },
        ),
    ];

    // When / Then
    for (offset, width, expected) in cases {
        assert_eq!(decode_write(offset, &vec![0xff; width]), Err(expected));
    }
}

#[test]
fn generated_action_and_boolean_masks_reject_reserved_bits() {
    for offset in [52, 108, 176, 192, 196, 200] {
        // Given
        let bytes = 0x101_u32.to_le_bytes();

        // When
        let result = decode_write(offset, &bytes);

        // Then
        assert_eq!(
            result,
            Err(MmioAccessError::ReservedBits {
                offset,
                bits: 0x100
            })
        );
    }
}

#[test]
fn dynamic_queue_access_has_no_template_extent_limit() {
    // Given
    let offset = queue_offset(1_000_000, QueueRegisters::SQ_BASE.offset()).expect("queue address");

    // When
    let result = decode_read(offset, 8);

    // Then
    assert!(
        matches!(result, Ok(RegisterLocation::Queue { index: 1_000_000, register })
        if register.id() == QueueRegistersRegisterId::SqBase)
    );
    assert!(decode_read(u32::MAX - 7, 8).is_ok());
}
