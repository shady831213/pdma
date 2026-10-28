use hw_model_schema::{SchemaCodecError, SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};

use super::{ModernHeader, ModernPacketPayloadCompactValues, ModernPacketPayloadWideValues};

#[test]
fn schema_location_when_copied_preserves_identity_and_coordinate() {
    // Given: two schema-local member identities.
    let payload = SchemaMemberIdentity::new("ModernPacket", "payload");
    let header = SchemaMemberIdentity::new("ModernPacket", "header");

    // When: one identity is placed at its root-relative byte coordinate.
    let location = payload.at(8);

    // Then: identity and coordinate remain separately comparable without allocation.
    assert_eq!(
        location,
        SchemaLocation::new(payload, SchemaCoordinate::Byte(8))
    );
    assert_eq!(location.identity(), payload);
    assert_eq!(location.identity().owner(), "ModernPacket");
    assert_eq!(location.identity().member(), "payload");
    assert_eq!(location.coordinate(), SchemaCoordinate::Byte(8));
    assert_ne!(location.identity(), header);
}

#[test]
fn schema_location_when_rebased_preserves_units_and_leaf_identity() {
    // Given: one leaf identity placed at byte and bit coordinates.
    let leaf = SchemaMemberIdentity::new("Flags", "mode");
    let byte = leaf.at_byte(3);
    let bit = leaf.at_bit(5);

    // When: both locations are rebased by byte and bit parent coordinates.
    let byte_by_byte = byte.checked_rebase(SchemaCoordinate::Byte(7));
    let bit_by_byte = bit.checked_rebase(SchemaCoordinate::Byte(7));
    let byte_by_bit = byte.checked_rebase(SchemaCoordinate::Bit(9));
    let bit_by_bit = bit.checked_rebase(SchemaCoordinate::Bit(9));

    // Then: byte-only composition stays byte-based and every bit composition is exact.
    assert_eq!(byte_by_byte, Some(leaf.at_byte(10)));
    assert_eq!(bit_by_byte, Some(leaf.at_bit(61)));
    assert_eq!(byte_by_bit, Some(leaf.at_bit(33)));
    assert_eq!(bit_by_bit, Some(leaf.at_bit(14)));
}

#[test]
fn coordinate_rebase_overflow_is_a_typed_schema_error() {
    // Given: a located semantic failure whose coordinate cannot be rebased.
    let location = SchemaMemberIdentity::new("Packet", "value").at_byte(usize::MAX);
    let error = SchemaCodecError::NarrowingOverflow {
        location,
        observed: 2,
        maximum: 1,
    };

    // When: the failure is rebased by another byte.
    let rebased = error.rebased(SchemaCoordinate::Byte(1));

    // Then: overflow is observable and retains the original leaf location and base.
    assert_eq!(
        rebased,
        SchemaCodecError::CoordinateRebaseOverflow {
            location,
            base: SchemaCoordinate::Byte(1),
        }
    );
}

#[test]
fn schema_codec_error_when_displayed_uses_schema_neutral_member_location_words() {
    // Given: a member-bearing schema error at one byte coordinate.
    let error = SchemaCodecError::UnknownTag {
        location: SchemaMemberIdentity::new("Packet", "opcode").at_byte(6),
        observed: 9,
    };

    // When: the error is rendered for a caller.
    let displayed = error.to_string();

    // Then: its owner.member identity and byte terminology are schema-neutral.
    assert_eq!(
        displayed,
        "schema member Packet.opcode at byte 6 has unknown tag 9"
    );
}

#[test]
fn generated_field_identities_distinguish_record_fields_and_union_leaves() {
    // Given: identities generated for direct record fields and tagged-union leaves.
    let opcode = ModernHeader::OPCODE_IDENTITY;
    let compact_count = ModernPacketPayloadCompactValues::COUNT_IDENTITY;
    let wide_second = ModernPacketPayloadWideValues::SECOND_IDENTITY;

    // When/Then: every identity retains its generated owner and declared field name.
    assert_eq!(opcode, SchemaMemberIdentity::new("ModernHeader", "opcode"));
    assert_eq!(
        compact_count,
        SchemaMemberIdentity::new("ModernPacketPayloadCompactValues", "count")
    );
    assert_eq!(
        wide_second,
        SchemaMemberIdentity::new("ModernPacketPayloadWideValues", "second")
    );
    assert_ne!(compact_count, wide_second);
}
