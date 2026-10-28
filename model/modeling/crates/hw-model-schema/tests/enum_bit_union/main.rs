//! Typed bitfield discriminants select union payloads without losing sibling bits.

/// Schemas shared by normalization and malformed-input cases.
pub mod schemas;
mod validation;

use hw_model_schema::SchemaCodecError;
use schemas::*;

#[test]
fn inline_enum_tag_normalizes_without_overwriting_sibling_fields() -> Result<(), SchemaCodecError> {
    // Given: the stored tag disagrees with the payload, alongside nonzero flags.
    let packet = Packet {
        prefix: 0xabcd,
        flags: PacketFlagsValues {
            ready: true,
            kind: PacketFlagsValuesKind::Small,
            priority: 5,
        },
        payload: PacketPayloadValues::Large(PacketPayloadLargeValues {
            first: 0x1234,
            second: 0x5678,
        }),
    };
    // When: the owned record is encoded and decoded through its public codec.
    let bytes = packet.to_bytes()?;
    let decoded = Packet::from_bytes(&bytes)?;
    // Then: only the tag changes, and independent wire bytes retain every field.
    assert_eq!(bytes, [0xcd, 0xab, 0xb1, 0, 0x34, 0x12, 0x78, 0x56]);
    assert_eq!(decoded.flags.kind, PacketFlagsValuesKind::Large);
    assert_eq!(decoded.flags.ready, packet.flags.ready);
    assert_eq!(decoded.flags.priority, packet.flags.priority);
    assert_eq!(decoded.payload, packet.payload);
    Ok(())
}

#[test]
fn named_bits_tag_selects_record_cases_with_big_endian_storage() -> Result<(), SchemaCodecError> {
    // Given: a qualified reusable header and a record-backed union.
    let packet = NamedPacket::new(
        0xabcd,
        named::HeaderValues {
            ready: true,
            priority: 5,
            kind: named::HeaderKind::Small,
        },
        NamedPacketPayloadValues::Large(Large {
            first: 0x1234,
            second: 0x5678,
        }),
    );
    // When: constructor normalization and the record codec select the Large case.
    let bytes = packet.to_bytes()?;
    let decoded = NamedPacket::from_bytes(&bytes)?;
    // Then: the packed tag and unrelated bits survive big-endian encoding.
    assert_eq!(bytes, [0xab, 0xcd, 2, 0x0b, 0x12, 0x34, 0x56, 0x78]);
    assert_eq!(packet.header.kind, named::HeaderKind::Large);
    assert_eq!(decoded, packet);
    Ok(())
}

#[test]
fn two_unions_normalize_their_own_tags_in_one_word() -> Result<(), SchemaCodecError> {
    // Given: opposite union choices and a separate flag share one control byte.
    let pair = Pair::new(
        PairTagsValues {
            left: PairTagsValuesLeft::Second,
            right: PairTagsValuesRight::First,
            ready: true,
        },
        PairLeftValues::First(PairLeftFirstValues { value: 17 }),
        PairRightValues::Second(PairRightSecondValues { value: 29 }),
    );
    // When: both tags are derived from their payloads.
    let bytes = pair.to_bytes()?;
    // Then: normalizing one tag never overwrites the other tag or the ready flag.
    assert_eq!(bytes, [0x84, 17, 29]);
    assert_eq!(Pair::from_bytes(&bytes)?, pair);
    Ok(())
}
