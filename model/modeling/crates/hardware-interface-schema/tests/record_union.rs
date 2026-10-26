//! Record-backed union branches and independent inline union storage.

use hardware_interface_schema::{SchemaCodecError, record_schema};

record_schema! { pub record Small { endian: Little, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { first: u64, second: u64 } } }
record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            tag: enum(u8) { variants: { Small = 0, Large = 1 } },
            payload: tagged_union { tag: self.tag, cases: { Small(Small), Large(Large) } },
        },
    }
}

record_schema! { pub record Tags { endian: Little, fields: {
    left: enum(u8) { variants: { A = 0, B = 1 } },
    right: enum(u8) { variants: { A = 0, B = 1 } },
} } }
record_schema! { pub record Pair { endian: Little, fields: {
    tags: Tags,
    left: tagged_union { tag: self.tags.left, cases: { A { offset: u64 }, B { index: u64, offset: u64 } } },
    right: tagged_union { tag: self.tags.right, cases: { A { offset: u64 }, B { index: u64, offset: u64 } } },
} } }

#[test]
fn independent_unions_have_independent_storage() -> Result<(), Box<dyn std::error::Error>> {
    // Given: two independently tagged unions occupy one record.
    let mut value = Pair::ZERO;
    value.right = PairRightValues::B(PairRightBValues {
        index: 7,
        offset: 9,
    });
    // When: the record is encoded and viewed through generated accessors.
    let bytes = value.to_bytes()?;
    let view = PairView::new(&bytes)?;
    // Then: word names and coordinates are scoped to their respective unions.
    assert_eq!(view.left_word0()?, 0);
    assert_eq!(view.right_word0()?, 7);
    assert_eq!(view.right_word1()?, 9);
    assert_eq!(Pair::from_bytes(&bytes)?.right, value.right);
    Ok(())
}

#[test]
fn record_cases_reuse_their_layout_and_reject_unused_storage()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: records with distinct sizes and alignments share one discriminated slot.
    let small = Packet::new(
        PacketTag::Large,
        PacketPayloadValues::Small(Small { value: 0x1234 }),
    );
    let large = Packet::new(
        PacketTag::Small,
        PacketPayloadValues::Large(Large {
            first: u64::MAX,
            second: 42,
        }),
    );
    // When: each selected record is encoded through the parent union.
    let mut bytes = small.to_bytes()?;
    // Then: tags normalize, nested values round trip, and inactive storage is canonical.
    assert_eq!(Packet::RECORD_SIZE, 24);
    assert_eq!(Packet::PAYLOAD_OFFSET, 8);
    assert!(Packet::SPEC.resolve().is_ok());
    assert_eq!(bytes[0], 0);
    assert_eq!(&bytes[8..10], &[0x34, 0x12]);
    assert_eq!(Packet::from_bytes(&bytes)?, small);
    assert_eq!(Packet::from_bytes(&large.to_bytes()?)?, large);
    bytes[10] = 1;
    assert!(matches!(
        Packet::from_bytes(&bytes),
        Err(SchemaCodecError::NonZeroReserved { observed: 1, .. })
    ));
    Ok(())
}
