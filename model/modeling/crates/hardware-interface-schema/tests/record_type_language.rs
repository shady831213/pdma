//! Modern record type-language behavior.

use hardware_interface_schema::{Decode, Encode, SchemaCodecError, record_schema};

#[path = "record_type_language/location.rs"]
mod location;

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
fn nested_record_when_child_enum_is_malformed_reports_the_root_byte_coordinate()
-> Result<(), SchemaCodecError> {
    // Given: a child record at a nonzero parent offset with an invalid enum discriminant.
    let mut bytes = ModernEnvelopeValues::ZERO.to_bytes()?;
    bytes[6] = 9;

    // When: the parent-owned codec decodes the malformed nested child.
    let result = ModernEnvelopeValues::from_bytes(&bytes);

    // Then: the child leaf identity is preserved and its coordinate is parent-relative.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: ModernHeader::OPCODE_IDENTITY.at_byte(6),
            observed: 9,
        })
    );
    Ok(())
}

#[test]
fn nested_record_when_child_reserved_bits_are_malformed_reports_the_root_bit_coordinate()
-> Result<(), SchemaCodecError> {
    // Given: a child packed-bits field at byte seven with its local reserved bit set.
    let mut bytes = ModernEnvelopeValues::ZERO.to_bytes()?;
    bytes[7] = 0x80;

    // When: the parent-owned codec decodes the malformed nested child.
    let result = ModernEnvelopeValues::from_bytes(&bytes);

    // Then: the child bits leaf is retained at root-relative bit 63.
    assert_eq!(
        result,
        Err(SchemaCodecError::NonZeroReserved {
            location: ModernHeaderFlagsValues::RESERVED_IDENTITY.at_bit(63),
            observed: 0x80,
        })
    );
    Ok(())
}

#[test]
fn nested_record_when_union_leaf_is_malformed_is_rebased_exactly_once()
-> Result<(), SchemaCodecError> {
    // Given: a nested compact union whose second word overflows its u8 semantic range.
    let mut bytes = ModernEnvelopeValues::ZERO.to_bytes()?;
    bytes[14..16].copy_from_slice(&0x0100_u16.to_le_bytes());

    // When: the outer record decodes the nested union member.
    let result = ModernEnvelopeValues::from_bytes(&bytes);

    // Then: the union leaf stays intact and moves from local byte 10 to root byte 14.
    assert_eq!(
        result,
        Err(SchemaCodecError::NarrowingOverflow {
            location: ModernPacketPayloadCompactValues::COUNT_IDENTITY.at_byte(14),
            observed: 0x0100,
            maximum: 0xff,
        })
    );
    Ok(())
}

#[test]
fn nested_record_when_child_bits_cannot_encode_keeps_destination_unchanged() {
    // Given: an overflowing nested packed-bits value and caller-owned sentinel bytes.
    let values = ModernEnvelopeValues {
        prefix: 0,
        packet: ModernPacketValues {
            header: ModernHeaderValues {
                version: 0,
                opcode: ModernHeaderOpcode::Compact,
                flags: ModernHeaderFlagsValues {
                    flag: false,
                    mode: 4,
                },
                command_id: 0,
            },
            payload: ModernPacketPayloadValues::Compact(ModernPacketPayloadCompactValues {
                value: 0,
                count: 0,
            }),
        },
    };
    let mut destination = [0xa5; ModernEnvelope::RECORD_SIZE];
    let before = destination;

    // When: the outer record attempts to encode the invalid nested value.
    let result = values.encode_into(&mut destination);

    // Then: the bits leaf is rebased once and no caller-owned byte commits.
    assert_eq!(
        result,
        Err(SchemaCodecError::NarrowingOverflow {
            location: ModernHeaderFlagsValues::MODE_IDENTITY.at_bit(57),
            observed: 4,
            maximum: 3,
        })
    );
    assert_eq!(destination, before);
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
        payload: ModernPacketPayloadValues::Compact(ModernPacketPayloadCompactValues {
            value: 0x1122,
            count: 7,
        }),
    };

    // When: the generated owned codec encodes and decodes the composed record.
    let bytes = values.to_bytes()?;
    let decoded = ModernPacketValues::from_bytes(&bytes)?;

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
fn typed_raw_when_converted_uses_the_recursive_record_codec() -> Result<(), SchemaCodecError> {
    // Given: one independently specified raw packet with nested header, enum, bits, and union.
    let bytes = [7, 0, 1, 5, 12, 0, 0x44, 0x33, 0x22, 0x11, 7, 0];
    let raw = ModernPacketRaw::from(bytes);

    // When: the raw owner and semantic value cross both generated codec surfaces.
    let decoded = raw.decode()?;
    let converted = ModernPacketValues::try_from(raw)?;
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
        ModernPacketValues::from_bytes(&unknown),
        ModernPacketValues::from_bytes(&reserved),
        ModernPacketValues::from_bytes(&constant),
        ModernPacketValues::from_bytes(&narrowed),
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
        Err(SchemaCodecError::NarrowingOverflow {
            location: ModernPacketPayloadCompactValues::COUNT_IDENTITY.at(10),
            observed: 0x0100,
            maximum: 0xff
        })
    );
}
