//! Explicit complete-record union behavior.
use hardware_interface_schema::{SchemaCodecError, record_schema, union_schema};

record_schema! { pub record Small { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { prefix: u8, value: u64 } } }
union_schema! { pub union Packet { cases: { Small(Small), Large(Large) } } }

#[test]
fn explicit_interpretation_of_untagged_bytes() {
    // Given bytes meaningful under either complete record.
    let bytes = [0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    // When the caller selects Small.
    let value = Packet::decode_variant(&bytes, PacketCase::Small).unwrap();
    // Then decoding uses the selected big-endian layout at byte zero.
    assert_eq!(value, Packet::Small(Small::new(0x1234)));
    assert_eq!(Packet::RECORD_SIZE, 16);
    assert_eq!(Packet::RECORD_ALIGNMENT, 8);
}

#[test]
fn tail_and_direct_gaps_are_reserved_only_in_union() {
    // Given nonzero ordinary record padding.
    let mut bytes = [0; 16];
    bytes[1] = 1;
    assert!(Large::from_bytes(&bytes).is_ok());
    // When selecting either union interpretation.
    let gap = Packet::decode_variant(&bytes, PacketCase::Large);
    bytes[1] = 0;
    bytes[15] = 1;
    let tail = Packet::decode_variant(&bytes, PacketCase::Small);
    // Then both padding violations are rejected.
    assert!(matches!(gap, Err(SchemaCodecError::NonZeroReserved { .. })));
    assert!(matches!(
        tail,
        Err(SchemaCodecError::NonZeroReserved { .. })
    ));
}

#[test]
fn encoding_clears_tail_and_preserves_outside_bytes() {
    // Given an oversized destination.
    let mut bytes = [0xff; 20];
    // When encoding the short case.
    Packet::Small(Small::new(0x1234))
        .encode_variant(&mut bytes)
        .unwrap();
    // Then the selected value starts at zero and its inactive tail is cleared.
    assert_eq!(&bytes[..2], &[0x12, 0x34]);
    assert_eq!(&bytes[2..16], &[0; 14]);
    assert_eq!(&bytes[16..], &[0xff; 4]);
}

#[test]
fn short_storage_rejects_atomically() {
    // Given a destination too short even for the short case's union.
    let mut bytes = [0xaa; 15];
    // When encoding.
    let result = Packet::ZERO.encode_variant(&mut bytes);
    // Then no bytes change and decoding also requires the entire union.
    assert!(matches!(
        result,
        Err(SchemaCodecError::BufferTooShort {
            required: 16,
            actual: 15
        })
    ));
    assert_eq!(bytes, [0xaa; 15]);
    assert!(Packet::decode_variant(&bytes, PacketCase::Small).is_err());
}

record_schema! {
    pub record Tagged {
        endian: Little,
        fields: {
            kind: enum(u8) { variants: { First = 1, Second = 2 } },
            payload: tagged_union { tag: self.kind, cases: {
                First { value: u16 }, Second { value: u16, other: u16 }
            } },
        }
    }
}
record_schema! {
    pub record Packed {
        endian: Little,
        fields: { flags: bits(u8) { fields: { value: 0..=2 } } }
    }
}
union_schema! { pub union Envelope { cases: { Tagged(Tagged), Packed(Packed) } } }

#[test]
fn selected_case_retains_nested_tagged_union_validation() {
    // Given a complete record containing a tagged union.
    let value = Tagged::new(
        TaggedKind::First,
        TaggedPayloadValues::Second(TaggedPayloadSecondValues {
            value: 0x1234,
            other: 0x5678,
        }),
    );
    let bytes = value.to_bytes().unwrap();
    // When its interpretation is explicitly selected.
    let decoded = Envelope::decode_variant(&bytes, EnvelopeCase::Tagged).unwrap();
    // Then the nested union still selects its own tag.
    assert_eq!(decoded, Envelope::Tagged(value));
}

#[test]
fn selected_case_rejects_unknown_enum_and_reserved_bits() {
    // Given invalid enum and reserved-bit storage.
    let mut unknown = [0; Envelope::RECORD_SIZE];
    unknown[0] = 3;
    let mut reserved = [0; Envelope::RECORD_SIZE];
    reserved[0] = 0x80;
    // When decoding the selected interpretations.
    let results = (
        Envelope::decode_variant(&unknown, EnvelopeCase::Tagged),
        Envelope::decode_variant(&reserved, EnvelopeCase::Packed),
    );
    // Then each record's own validation remains active.
    assert!(matches!(
        results.0,
        Err(SchemaCodecError::UnknownTag { .. })
    ));
    assert!(matches!(
        results.1,
        Err(SchemaCodecError::NonZeroReserved { .. })
    ));
}

#[test]
fn invalid_semantic_case_leaves_destination_unchanged() {
    // Given an out-of-range semantic bit field and existing bytes.
    let invalid = Envelope::Packed(Packed::new(PackedFlagsValues { value: 8 }));
    let mut bytes = [0xaa; Envelope::RECORD_SIZE];
    // When encoding fails inside the selected record.
    let result = invalid.encode_variant(&mut bytes);
    // Then even the active region is unchanged.
    assert!(matches!(
        result,
        Err(SchemaCodecError::NarrowingOverflow { .. })
    ));
    assert_eq!(bytes, [0xaa; Envelope::RECORD_SIZE]);
}

#[test]
fn raw_owner_and_views_keep_case_selection_explicit() {
    // Given raw owned storage and an explicit mutable view.
    let mut raw = PacketRaw::new([0xaa; Packet::RECORD_SIZE]);
    let value = Packet::Large(Large::new(7, 0x1234));
    // When replacing through the view.
    PacketViewMut::new(raw.as_mut())
        .unwrap()
        .set_values(&value)
        .unwrap();
    // Then an explicitly selected immutable view decodes the same value.
    assert_eq!(
        PacketView::new(raw.as_ref())
            .unwrap()
            .values(PacketCase::Large),
        Ok(value)
    );
    assert_eq!(raw.as_bytes(), &raw.into_bytes());
    assert_eq!(PacketRaw::default().into_bytes(), [0; 16]);
}

record_schema! { pub record Little { endian: Little, fields: { value: u16 } } }
union_schema! { pub union EndianChoice { cases: { Big(Small), Little(Little) } } }
record_schema! { pub record Twelve { endian: Little, fields: { a: u32, b: u32, c: u32 } } }
record_schema! { pub record Wide { endian: Little, fields: { value: u64 } } }
union_schema! { pub union Rounded { cases: { Twelve(Twelve), Wide(Wide) } } }

#[test]
fn identical_bytes_have_two_explicit_interpretations() {
    // Given bytes with no tag and two equally valid layouts.
    let bytes = [0x12, 0x34];
    // When callers independently choose their interpretation.
    let big = EndianChoice::decode_variant(&bytes, EndianChoiceCase::Big).unwrap();
    let little = EndianChoice::decode_variant(&bytes, EndianChoiceCase::Little).unwrap();
    // Then neither case selection is inferred from the bytes.
    assert_eq!(big, EndianChoice::Big(Small::new(0x1234)));
    assert_eq!(little, EndianChoice::Little(Little::new(0x3412)));
}

#[test]
fn largest_extent_is_rounded_to_another_cases_alignment() {
    // Given a twelve-byte record and a shorter eight-aligned record.
    let value = Rounded::Twelve(Twelve::new(1, 2, 3));
    let mut bytes = [0xff; Rounded::RECORD_SIZE];
    // When encoding the largest case.
    value.encode_variant(&mut bytes).unwrap();
    // Then union alignment adds four inactive bytes to its largest record.
    assert_eq!(bytes, [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(Rounded::RECORD_ALIGNMENT, 8);
}
