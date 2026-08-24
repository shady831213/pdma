//! Modern record type-language behavior.

use hardware_interface_schema::{SchemaCodecError, record_schema};

record_schema! {
    pub record ModernHeader {
        endian: Little,
        fields: {
            version: u16,
            opcode: enum(u8) {
                variants: {
                    Compact = 1,
                    Wide,
                },
            },
            flags: bits(u8) {
                fields: {
                    flag: 0..1,
                    mode: 1..3,
                },
            },
            size: u16 { const: ModernPacket::record_size },
            command_id: u16,
        },
    }
}

record_schema! {
    pub record ModernPacket {
        alignment: 4,
        endian: Little,
        fields: {
            header: ModernHeader,
            payload: union {
                tag: self.header.opcode,
                cases: {
                    Compact {
                        value: u16,
                        count: u8 { storage: u16 },
                    },
                    Wide {
                        first: u16,
                        second: u16,
                    },
                },
            },
        },
    }
}

#[test]
fn composed_types_when_encoded_derive_layout_tag_and_constant() -> Result<(), SchemaCodecError> {
    // Given: a semantic payload whose stale header tag disagrees with its union case.
    let values = ModernPacketValues {
        header: ModernHeaderValues {
            version: 7,
            opcode: ModernHeaderOpcode::Wide,
            flags: ModernHeaderFlagsValues {
                flag: true,
                mode: 2,
            },
            command_id: 0x3344,
        },
        payload: ModernPacketPayloadValues::Compact {
            value: 0x1122,
            count: 7,
        },
    };

    // When: the generated owned codec encodes and decodes the composed record.
    let bytes = values.to_bytes()?;
    let decoded = ModernPacketValues::from_bytes(&bytes)?;

    // Then: layout, derived fields, packed bits, narrowing, and exact bytes agree.
    assert_eq!(ModernHeader::RECORD_SIZE, 8);
    assert_eq!(ModernPacket::RECORD_SIZE, 12);
    assert_eq!(ModernPacket::SPEC.size(), 12);
    assert_eq!(ModernPacket::SPEC.alignment(), 4);
    assert_eq!(bytes, [7, 0, 1, 5, 12, 0, 0x44, 0x33, 0x22, 0x11, 7, 0]);
    assert_eq!(decoded.header.opcode, ModernHeaderOpcode::Compact);
    assert_eq!(
        decoded,
        ModernPacketValues {
            header: ModernHeaderValues {
                opcode: ModernHeaderOpcode::Compact,
                ..values.header
            },
            ..values
        }
    );
    Ok(())
}

#[test]
fn typed_boundaries_when_wire_values_are_invalid_report_exact_errors() {
    // Given: independently malformed enum, bits, constant, and narrowed storage values.
    let canonical = [7, 0, 1, 5, 12, 0, 0x44, 0x33, 0x22, 0x11, 7, 0];
    let mut unknown = canonical;
    unknown[2] = 9;
    let mut reserved = canonical;
    reserved[3] |= 0x80;
    let mut constant = canonical;
    constant[4..6].copy_from_slice(&11_u16.to_le_bytes());
    let mut narrowed = canonical;
    narrowed[10..12].copy_from_slice(&0x0100_u16.to_le_bytes());

    // When: each malformed value crosses the generated owned-codec boundary.
    let errors = [
        ModernPacketValues::from_bytes(&unknown),
        ModernPacketValues::from_bytes(&reserved),
        ModernPacketValues::from_bytes(&constant),
        ModernPacketValues::from_bytes(&narrowed),
    ];

    // Then: every structural failure retains its exact byte-level observation.
    assert_eq!(errors[0], Err(SchemaCodecError::UnknownTag { observed: 9 }));
    assert_eq!(
        errors[1],
        Err(SchemaCodecError::NonZeroReserved {
            offset: 3,
            observed: 0x80
        })
    );
    assert_eq!(
        errors[2],
        Err(SchemaCodecError::ConstantMismatch {
            offset: 4,
            observed: 11,
            expected: 12
        })
    );
    assert_eq!(
        errors[3],
        Err(SchemaCodecError::NarrowingOverflow {
            offset: 10,
            observed: 0x0100,
            maximum: 0xff
        })
    );
}
