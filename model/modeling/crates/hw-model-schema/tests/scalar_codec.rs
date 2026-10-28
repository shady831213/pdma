//! Scalar codec conformance tests.

use hw_model_schema::{Endian, ScalarKind, SchemaCodecError, read_scalar, write_scalar};

#[test]
fn scalar_u8_has_exact_bytes_and_round_trips_for_both_endians() {
    // Given
    let expected = [0xa5];

    for endian in [Endian::Little, Endian::Big] {
        // When
        let mut bytes = [0_u8; 1];
        let encoded = write_scalar(&mut bytes, endian, 0xa5_u8);
        let decoded = read_scalar::<u8>(&bytes, endian);

        // Then
        assert_eq!(encoded, Ok(()));
        assert_eq!(bytes, expected);
        assert_eq!(decoded, Ok(0xa5));
    }
}

#[test]
fn scalar_u16_has_exact_endian_bytes_and_round_trips() {
    // Given
    let cases = [(Endian::Little, [0x34, 0x12]), (Endian::Big, [0x12, 0x34])];

    for (endian, expected) in cases {
        // When
        let mut bytes = [0_u8; 2];
        let encoded = write_scalar(&mut bytes, endian, 0x1234_u16);
        let decoded = read_scalar::<u16>(&bytes, endian);

        // Then
        assert_eq!(encoded, Ok(()));
        assert_eq!(bytes, expected);
        assert_eq!(decoded, Ok(0x1234));
    }
}

#[test]
fn scalar_u32_has_exact_endian_bytes_and_round_trips() {
    // Given
    let cases = [
        (Endian::Little, [0x78, 0x56, 0x34, 0x12]),
        (Endian::Big, [0x12, 0x34, 0x56, 0x78]),
    ];

    for (endian, expected) in cases {
        // When
        let mut bytes = [0_u8; 4];
        let encoded = write_scalar(&mut bytes, endian, 0x1234_5678_u32);
        let decoded = read_scalar::<u32>(&bytes, endian);

        // Then
        assert_eq!(encoded, Ok(()));
        assert_eq!(bytes, expected);
        assert_eq!(decoded, Ok(0x1234_5678));
    }
}

#[test]
fn scalar_u64_has_exact_endian_bytes_and_round_trips() {
    // Given
    let cases = [
        (
            Endian::Little,
            [0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
        ),
        (
            Endian::Big,
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef],
        ),
    ];

    for (endian, expected) in cases {
        // When
        let mut bytes = [0_u8; 8];
        let encoded = write_scalar(&mut bytes, endian, 0x0123_4567_89ab_cdef_u64);
        let decoded = read_scalar::<u64>(&bytes, endian);

        // Then
        assert_eq!(encoded, Ok(()));
        assert_eq!(bytes, expected);
        assert_eq!(decoded, Ok(0x0123_4567_89ab_cdef));
    }
}

#[test]
fn scalar_write_rejects_short_destination_without_mutation() {
    // Given
    let mut bytes = [0xcc; 3];

    // When
    let result = write_scalar(&mut bytes, Endian::Little, 0x1234_5678_u32);

    // Then
    assert_eq!(
        result,
        Err(SchemaCodecError::BufferTooShort {
            required: 4,
            actual: 3,
        })
    );
    assert_eq!(bytes, [0xcc; 3]);
}

#[test]
fn scalar_read_rejects_short_source() {
    // Given
    let bytes = [0_u8; 7];

    // When
    let result = read_scalar::<u64>(&bytes, Endian::Big);

    // Then
    assert_eq!(
        result,
        Err(SchemaCodecError::BufferTooShort {
            required: 8,
            actual: 7,
        })
    );
}

#[test]
fn scalar_kinds_report_their_wire_widths() {
    // Given / When / Then
    assert_eq!(ScalarKind::U8.width(), 1);
    assert_eq!(ScalarKind::U16.width(), 2);
    assert_eq!(ScalarKind::U32.width(), 4);
    assert_eq!(ScalarKind::U64.width(), 8);
}
