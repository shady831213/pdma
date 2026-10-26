use super::schemas::*;
use hardware_interface_schema::SchemaCodecError;

#[test]
fn unknown_inline_tag_reports_its_absolute_bit_coordinate() {
    // Given: tag3 at bit3 of a control word starting at byte2.
    let bytes = [0, 0, 0x18, 0, 0, 0, 0, 0];
    // When: a raw packet crosses the typed decoding boundary.
    let result = Packet::from_bytes(&bytes);
    // Then: the error identifies the enum member rather than the union storage.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: PacketFlagsValues::KIND_IDENTITY.at_bit(19),
            observed: 3,
        })
    );
}

#[test]
fn unknown_named_tag_keeps_identity_and_big_endian_bit_coordinate() {
    // Given: tag3 in a reusable header at byte2, with its tag starting at bit8.
    let bytes = [0, 0, 3, 0, 0, 0, 0, 0];
    // When: a record-backed union's tag is decoded.
    let result = NamedPacket::from_bytes(&bytes);
    // Then: the reusable enum retains its identity and absolute logical bit position.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: named::Header::KIND_IDENTITY.at_bit(24),
            observed: 3,
        })
    );
}

#[test]
fn tag_selection_does_not_hide_reserved_control_bits() {
    // Given: assigned Small tags with one nonzero reserved bit in each control word.
    let inline = [0, 0, 0x0c, 0, 0, 0, 0, 0];
    let named = [0, 0, 0x81, 0, 0, 0, 0, 0];
    // When: each typed record decodes its header before selecting the payload.
    let results = (Packet::from_bytes(&inline), NamedPacket::from_bytes(&named));
    // Then: both representations retain their existing reserved-bit checks.
    assert_eq!(
        results.0,
        Err(SchemaCodecError::NonZeroReserved {
            location: PacketFlagsValues::RESERVED_IDENTITY.at_bit(18),
            observed: 4,
        })
    );
    assert_eq!(
        results.1,
        Err(SchemaCodecError::NonZeroReserved {
            location: NamedPacket::HEADER_IDENTITY.at_bit(31),
            observed: 0x8000,
        })
    );
}

#[test]
fn small_cases_reject_nonzero_inactive_payload_bytes() {
    // Given: valid Small tags but a nonzero second payload word, unused by Small.
    let inline = [0, 0, 8, 0, 0, 0, 1, 0];
    let named = [0, 0, 1, 0, 0, 0, 1, 0];
    // When: both forms decode the selected smaller case.
    let results = (Packet::from_bytes(&inline), NamedPacket::from_bytes(&named));
    // Then: changing the discriminator representation does not weaken union validation.
    assert!(matches!(
        results.0,
        Err(SchemaCodecError::NonZeroReserved { .. })
    ));
    assert!(matches!(
        results.1,
        Err(SchemaCodecError::NonZeroReserved { .. })
    ));
}

#[test]
fn failed_encoding_preserves_the_destination_and_rejects_bad_sibling_bits() {
    // Given: a valid payload and an out-of-range priority alongside the tag.
    let mut packet = Packet::ZERO;
    packet.flags.priority = 8;
    let mut destination = [0xa5; Packet::RECORD_SIZE];
    // When: normalization is followed by checked encoding into caller storage.
    let result = packet.encode_into(&mut destination);
    // Then: unrelated field validation still fails before any destination byte changes.
    assert_eq!(
        result,
        Err(SchemaCodecError::NarrowingOverflow {
            location: PacketFlagsValues::PRIORITY_IDENTITY.at_bit(21),
            observed: 8,
            maximum: 7,
        })
    );
    assert_eq!(destination, [0xa5; Packet::RECORD_SIZE]);
}
