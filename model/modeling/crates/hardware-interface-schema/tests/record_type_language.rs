//! Modern record type-language behavior.

use hardware_interface_schema::{Decode, Encode, SchemaCodecError, bits_schema, record_schema};

#[path = "record_type_language/location.rs"]
mod location;
#[path = "record_type_language/nested.rs"]
mod nested;

bits_schema! {
    pub bits U8Word(u16) {
        value: 0..8,
    }
}

record_schema! {
    pub record NamedWordPacket {
        endian: Little,
        fields: {
            opcode: enum(u16) {
                variants: {
                    Only = 1,
                },
            },
            payload: union {
                tag: self.opcode,
                cases: {
                    Only {
                        count: U8Word,
                    },
                },
            },
        },
    }
}

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
                        count: U8Word,
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

record_schema! {
    pub record ModernEnvelope {
        alignment: 4,
        endian: Little,
        fields: {
            prefix: u32,
            packet: ModernPacket,
        },
    }
}

#[test]
fn composed_types_when_encoded_derive_layout_tag_and_constant() -> Result<(), SchemaCodecError> {
    // Given: a semantic payload whose stale header tag disagrees with its union case.
    let values = ModernPacket {
        header: ModernHeader {
            version: 7,
            opcode: ModernHeaderOpcode::Wide,
            flags: ModernHeaderFlagsValues {
                flag: true,
                mode: 2,
            },
            command_id: 0x3344,
        },
        payload: ModernPacketPayloadValues::Compact(ModernPacketPayloadCompactValues {
            value: 0x1122,
            count: U8WordValues { value: 7 },
        }),
    };

    // When: the generated owned codec encodes and decodes the composed record.
    let bytes = values.to_bytes()?;
    let decoded = ModernPacket::from_bytes(&bytes)?;

    // Then: layout, derived fields, packed bits, narrowing, and exact bytes agree.
    assert_eq!(ModernHeader::RECORD_SIZE, 8);
    assert_eq!(ModernPacket::RECORD_SIZE, 12);
    assert_eq!(ModernPacket::SPEC.size(), 12);
    assert_eq!(ModernPacket::SPEC.alignment(), 4);
    assert_eq!(values.payload.tag(), ModernHeaderOpcode::Compact);
    assert_eq!(bytes, [7, 0, 1, 5, 12, 0, 0x44, 0x33, 0x22, 0x11, 7, 0]);
    assert_eq!(decoded.header.opcode, ModernHeaderOpcode::Compact);
    assert_eq!(
        decoded,
        ModernPacket {
            header: ModernHeader {
                opcode: ModernHeaderOpcode::Compact,
                ..values.header
            },
            ..values
        }
    );
    Ok(())
}

#[test]
fn typed_raw_when_converted_uses_the_recursive_record_codec() -> Result<(), SchemaCodecError> {
    // Given: one independently specified raw packet with nested header, enum, bits, and union.
    let bytes = [7, 0, 1, 5, 12, 0, 0x44, 0x33, 0x22, 0x11, 7, 0];
    let raw = ModernPacketRaw::from(bytes);

    // When: the raw owner and semantic value cross both generated codec surfaces.
    let decoded = raw.decode()?;
    let converted = ModernPacket::try_from(raw)?;
    let encoded = decoded.encode()?;
    let converted_raw = ModernPacketRaw::try_from(converted)?;

    // Then: every path delegates to the same exact recursive wire representation.
    assert_eq!(decoded, converted);
    assert_eq!(encoded.as_bytes(), &bytes);
    assert_eq!(converted_raw.as_bytes(), &bytes);
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
        ModernPacket::from_bytes(&unknown),
        ModernPacket::from_bytes(&reserved),
        ModernPacket::from_bytes(&constant),
        ModernPacket::from_bytes(&narrowed),
    ];

    // Then: every structural failure retains its exact byte-level observation.
    assert_eq!(
        errors[0],
        Err(SchemaCodecError::UnknownTag {
            location: ModernHeader::OPCODE_IDENTITY.at(2),
            observed: 9
        })
    );
    assert_eq!(
        errors[1],
        Err(SchemaCodecError::NonZeroReserved {
            location: ModernHeaderFlagsValues::RESERVED_IDENTITY.at_bit(31),
            observed: 0x80
        })
    );
    assert_eq!(
        errors[2],
        Err(SchemaCodecError::ConstantMismatch {
            location: ModernHeader::SIZE_IDENTITY.at(4),
            observed: 11,
            expected: 12
        })
    );
    assert_eq!(
        errors[3],
        Err(SchemaCodecError::NonZeroReserved {
            location: ModernPacketPayloadCompactValues::COUNT_IDENTITY.at_bit(88),
            observed: 0x0100,
        })
    );
}
