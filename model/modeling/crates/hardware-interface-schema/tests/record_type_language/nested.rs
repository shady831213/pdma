use super::*;

#[test]
fn nested_record_when_child_enum_is_malformed_reports_the_root_byte_coordinate()
-> Result<(), SchemaCodecError> {
    // Given: a child record at a nonzero parent offset with an invalid enum discriminant.
    let mut bytes = ModernEnvelope::ZERO.to_bytes()?;
    bytes[6] = 9;

    // When: the parent-owned codec decodes the malformed nested child.
    let result = ModernEnvelope::from_bytes(&bytes);

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
fn named_bits_when_used_directly_in_union_matches_its_inline_layout() -> Result<(), SchemaCodecError>
{
    // Given: one union case containing a reusable u8 value in a u16 container.
    let packet = NamedWordPacket {
        opcode: NamedWordPacketOpcode::Only,
        payload: NamedWordPacketPayloadValues::Only(NamedWordPacketPayloadOnlyValues {
            count: U8WordValues { value: 7 },
        }),
    };

    // When: the generated record codec crosses the named field boundary.
    let encoded = packet.to_bytes()?;
    let decoded = NamedWordPacket::from_bytes(&encoded)?;

    // Then: the named field occupies its full container and round-trips unchanged.
    assert_eq!(encoded, [1, 0, 7, 0]);
    assert_eq!(decoded, packet);
    Ok(())
}

#[test]
fn nested_record_when_child_reserved_bits_are_malformed_reports_the_root_bit_coordinate()
-> Result<(), SchemaCodecError> {
    // Given: a child packed-bits field at byte seven with its local reserved bit set.
    let mut bytes = ModernEnvelope::ZERO.to_bytes()?;
    bytes[7] = 0x80;

    // When: the parent-owned codec decodes the malformed nested child.
    let result = ModernEnvelope::from_bytes(&bytes);

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
    let mut bytes = ModernEnvelope::ZERO.to_bytes()?;
    bytes[14..16].copy_from_slice(&0x0100_u16.to_le_bytes());

    // When: the outer record decodes the nested union member.
    let result = ModernEnvelope::from_bytes(&bytes);

    // Then: the union leaf stays intact and moves from local byte 10 to root byte 14.
    assert_eq!(
        result,
        Err(SchemaCodecError::NonZeroReserved {
            location: ModernPacketPayloadCompactValues::COUNT_IDENTITY.at_bit(120),
            observed: 0x0100,
        })
    );
    Ok(())
}

#[test]
fn nested_record_when_child_bits_cannot_encode_keeps_destination_unchanged() {
    // Given: an overflowing nested packed-bits value and caller-owned sentinel bytes.
    let values = ModernEnvelope {
        prefix: 0,
        packet: ModernPacket {
            header: ModernHeader {
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
                count: U8WordValues::ZERO,
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
