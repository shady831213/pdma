//! Generated owned record-value behavior.

use hardware_interface_schema::{Decode, Encode, SchemaCodecError, field_schema, record_schema};

field_schema! { pub field LeWord { scalar: u32, endian: Little } }

#[test]
fn reusable_field_raw_when_converted_owns_its_declared_endian() -> Result<(), SchemaCodecError> {
    // Given: one little-endian raw field owner.
    let raw = LeWordRaw::from([0x04, 0x03, 0x02, 0x01]);

    // When: the generated field codec crosses both directions.
    let value = raw.decode()?;
    let encoded = value.encode()?;

    // Then: the field itself applies endian and preserves the exact bytes.
    assert_eq!(value.value(), 0x0102_0304);
    assert_eq!(encoded.as_bytes(), &[0x04, 0x03, 0x02, 0x01]);
    Ok(())
}

record_schema! {
    pub record FlatRecord {
        size: 16,
        alignment: 8,
        fields: {
            tag: u16 { id: 1, offset: 0, endian: Little },
            lanes: [u32; 2] { ids: [2, 3], offset: 4, stride: 4, endian: Little },
        }
    }
}

record_schema! {
    pub record HeaderRecord {
        size: 8,
        alignment: 4,
        fields: {
            sequence: LeWord { id: 1, offset: 0 },
            flags: u16 { id: 2, offset: 4, endian: Big },
        },
        records: {},
    }
}

record_schema! {
    pub record NestedRecord {
        size: 24,
        alignment: 8,
        fields: {
            kind: u16 { id: 1, offset: 0, endian: Little },
        },
        records: {
            header: HeaderRecord { id: 2, offset: 8 },
        },
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
        size: 8,
        alignment: 4,
        fields: {
            prefix: u32 { id: 1, offset: 0, endian: Little },
        },
        records: {
            child: ComposableErrorChild { id: 2, offset: 4 },
        },
    }
}

#[test]
fn owned_values_round_trip_scalars_arrays_references_and_nested_records()
-> Result<(), SchemaCodecError> {
    // Given: owned values spanning every record placement shape and nonzero padding.
    let flat = FlatRecordValues {
        tag: 0x1122,
        lanes: [0x3344_5566, 0x7788_99aa],
    };
    let nested = NestedRecordValues {
        kind: 0xa1b2,
        header: HeaderRecordValues {
            sequence: 0x0102_0304,
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
            0xb2, 0xa1, 0, 0, 0, 0, 0, 0, 0x04, 0x03, 0x02, 0x01, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ]
    );
    assert_eq!(FlatRecordValues::from_bytes(&flat_bytes)?, flat);
    assert_eq!(NestedRecordValues::from_bytes(&nested_bytes)?, nested);
    assert_eq!(FlatRecordValues::ZERO, FlatRecordValues::default());
    Ok(())
}

#[test]
fn owned_values_reject_short_output_before_mutation() {
    // Given: a short caller-owned buffer filled with a visible sentinel.
    let mut bytes = [0xa5; 15];
    let before = bytes;

    // When: a generated owned value attempts a complete encoding.
    let result = FlatRecordValues::ZERO.encode_into(&mut bytes);

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
    let mut bytes = ComposableErrorParentValues::ZERO.to_bytes()?;
    bytes[4] = 9;

    // When: the parent decodes the malformed nested child.
    let result = ComposableErrorParentValues::from_bytes(&bytes);

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
