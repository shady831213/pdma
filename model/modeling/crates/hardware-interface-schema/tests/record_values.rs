//! Generated owned record-value behavior.

use hardware_interface_schema::{SchemaCodecError, record_schema};

#[test]
fn raw_record_exposes_bytes_without_decoding_them() {
    // Given: the semantic enum rejects zero, but raw bytes remain unrestricted.
    let mut raw = ComposableErrorChildRaw::default();
    // When
    let bytes: &mut [u8] = raw.as_mut();
    bytes.copy_from_slice(&[0xff]);
    let bytes: &[u8] = raw.as_ref();
    // Then
    assert_eq!(bytes, &[0xff]);
    assert!(matches!(
        ComposableErrorChild::from_bytes(bytes),
        Err(SchemaCodecError::UnknownTag { observed: 255, .. })
    ));
}

record_schema! {
    pub record FlatRecord {
        alignment: 8,
        endian: Little,
        fields: {
            tag: u16,
            lane0: u32,
            lane1: u32,
        }
    }
}

record_schema! {
    pub record LittleSequence {
        alignment: 4,
        endian: Little,
        fields: {
            value: u32,
        }
    }
}

record_schema! {
    pub record HeaderRecord {
        alignment: 4,
        endian: Big,
        fields: {
            sequence: LittleSequence,
            flags: u16,
        }
    }
}

record_schema! {
    pub record NestedRecord {
        alignment: 8,
        endian: Little,
        fields: {
            kind: u16,
            padding: u32,
            header: HeaderRecord,
        }
    }
}

record_schema! {
    pub record ComposableErrorChild {
        endian: Little,
        fields: {
            kind: enum(u8) {
                variants: {
                    Known = 1,
                },
            },
        },
    }
}

record_schema! {
    pub record ComposableErrorParent {
        alignment: 4,
        endian: Little,
        fields: {
            prefix: u32,
            child: ComposableErrorChild,
        }
    }
}

#[test]
fn owned_values_round_trip_scalars_arrays_references_and_nested_records()
-> Result<(), SchemaCodecError> {
    // Given: owned values spanning every record placement shape and nonzero padding.
    let flat = FlatRecord {
        tag: 0x1122,
        lane0: 0x3344_5566,
        lane1: 0x7788_99aa,
    };
    let nested = NestedRecord {
        kind: 0xa1b2,
        padding: 0,
        header: HeaderRecord {
            sequence: LittleSequence { value: 0x0102_0304 },
            flags: 0x0506,
        },
    };

    // When: generated owned codecs encode and decode complete records.
    let flat_bytes = flat.to_bytes()?;
    let nested_bytes = nested.to_bytes()?;

    // Then: fields use declared endian order, padding is zero, and values round-trip exactly.
    assert_eq!(
        flat_bytes,
        [
            0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33, 0xaa, 0x99, 0x88, 0x77, 0, 0, 0, 0,
        ]
    );
    assert_eq!(
        nested_bytes,
        [
            0xb2, 0xa1, 0, 0, 0, 0, 0, 0, 0x04, 0x03, 0x02, 0x01, 0x05, 0x06, 0, 0,
        ]
    );
    assert_eq!(FlatRecord::from_bytes(&flat_bytes)?, flat);
    assert_eq!(NestedRecord::from_bytes(&nested_bytes)?, nested);
    assert_eq!(FlatRecord::ZERO, FlatRecord::default());
    Ok(())
}

#[test]
fn owned_values_reject_short_output_before_mutation() {
    // Given: a short caller-owned buffer filled with a visible sentinel.
    let mut bytes = [0xa5; 15];
    let before = bytes;

    // When: a generated owned value attempts a complete encoding.
    let result = FlatRecord::ZERO.encode_into(&mut bytes);

    // Then: extent validation fails before padding or fields are written.
    assert_eq!(
        result,
        Err(SchemaCodecError::BufferTooShort {
            required: 16,
            actual: 15,
        })
    );
    assert_eq!(bytes, before);
}

#[test]
fn composable_record_when_nested_modern_child_is_malformed_rebases_the_child_location()
-> Result<(), SchemaCodecError> {
    // Given: a composable parent with a modern enum child at nonzero byte offset four.
    let mut bytes = ComposableErrorParent::ZERO.to_bytes()?;
    bytes[4] = 9;

    // When: the parent decodes the malformed nested child.
    let result = ComposableErrorParent::from_bytes(&bytes);

    // Then: the child identity remains intact at the parent-root byte coordinate.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: ComposableErrorChild::KIND_IDENTITY.at_byte(4),
            observed: 9,
        })
    );
    Ok(())
}
