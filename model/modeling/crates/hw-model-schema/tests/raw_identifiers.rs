//! Raw Rust spelling does not leak into derived helper identifiers.

use hw_model_schema::{
    Decode, Encode, SchemaMemberIdentity, bits_schema, record_schema, register_map,
    register_schema, union_schema,
};

bits_schema! { pub bits r#RawBits(u8) { r#type: 0..1, r#match: 1..4 } }
register_schema! {
    pub register r#RawRegister {
        width: 1, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 15,
        fields: { r#type: 0..1, r#match: 1..4 },
    }
}
register_map! {
    pub map r#RawMap {
        alignment: 1,
        placements: [pub r#type: r#RawRegister, pub r#match: r#RawRegister[2]],
        access: [r#mod.r#use => r#type, r#mod.r#loop => r#match],
    }
}
register_map! {
    pub map r#RawInstance(r#type: usize) {
        alignment: 1, placements: [pub r#match: r#RawMap[r#type]],
    }
}
record_schema! {
    pub record r#RawRecord {
        endian: Little,
        fields: {
            r#type: bits(u8) { fields: { r#match: 0..4 } },
            r#mod: bits(r#RawBits),
            r#enum: enum(u8) { variants: { r#Inline = 1 } },
            r#union: tagged_union {
                tag: self.r#enum,
                cases: {
                    r#Inline { r#type: u8, r#match: r#RawBits },
                },
            },
        },
    }
}
record_schema! {
    pub record r#RawPayload { endian: Big, fields: { r#type: u16 } }
}
union_schema! { pub union r#RawUnion { cases: { r#Named(r#RawPayload) } } }

bits_schema! {
    pub bits r#RawEnumBits(u8) {
        r#type: 0..2 => enum { variants: { r#Idle = 0, r#Busy = 2 } },
    }
}
record_schema! {
    pub record r#RawCodecRecord {
        endian: Little,
        fields: {
            r#enum: enum(u8) { variants: { r#Named = 2 } },
            r#union: tagged_union {
                tag: self.r#enum, cases: { r#Named(r#RawPayload) },
            },
        },
    }
}

#[test]
fn raw_bits_when_encoded_keep_values_and_declared_identity()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: raw field and type names, including a narrow integer field.
    let value = RawBitsValues {
        r#type: true,
        r#match: 5,
    };
    // When: generated raw and semantic codecs round-trip the value.
    let raw = value.encode()?;
    let decoded = raw.decode()?;
    // Then: the wire representation and existing schema spelling are retained.
    assert_eq!(raw.value(), 11);
    assert_eq!(decoded, value);
    assert_eq!(
        RawBits::TYPE_IDENTITY,
        SchemaMemberIdentity::new("r#RawBits", "r#type")
    );
    assert_ne!(RawBitsFieldId::Type, RawBitsFieldId::Match);
    Ok(())
}

#[test]
fn raw_register_and_map_when_accessed_reach_the_same_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: singleton and repeated raw placements sharing one byte image.
    let mut bytes = [0; 3];
    // When: grouped and flat generated accessors write raw-named fields.
    RawMapViewMut::new(&mut bytes)?
        .mod_mut()?
        .use_mut()?
        .set_type(true)?;
    RawMapViewMut::new(&mut bytes)?.match_mut(1)?.set_match(5)?;
    // Then: both paths expose the expected independent register slots.
    let view = RawMapView::new(&bytes)?;
    assert!(view.r#type()?.r#type()?);
    assert_eq!(view.r#mod()?.r#loop(1)?.r#match()?, 5);
    assert_eq!(bytes, [1, 0, 10]);
    assert_eq!(RawRegisterValues::ZERO.with_match(5)?.raw(), 10);
    assert_ne!(RawRegisterFieldId::Type, RawRegisterFieldId::Match);
    let layout = RawInstance::new(2)?;
    assert_eq!(layout.extent(), 6);
    assert_eq!(layout.r#match().count(), 2);
    assert_eq!(layout.resolve()?.registers().len(), 6);
    Ok(())
}

#[test]
fn raw_record_members_when_inline_union_selected_round_trip()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: inline bits, a named bits reference and raw union case members.
    let value = RawRecord::new(
        RawRecordTypeValues { r#match: 9 },
        RawBitsValues {
            r#type: true,
            r#match: 3,
        },
        RawRecordEnum::r#Inline,
        RawRecordUnionValues::r#Inline(RawRecordUnionInlineValues {
            r#type: 0x34,
            r#match: RawBitsValues {
                r#type: false,
                r#match: 2,
            },
        }),
    );
    // When: the generated record codec serializes and decodes raw names.
    let bytes = value.to_bytes()?;
    let decoded = RawRecord::from_bytes(&bytes)?;
    // Then: all members survive with literal wire bytes.
    assert_eq!(bytes, [9, 7, 1, 0x34, 4]);
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
fn raw_union_case_when_named_codec_selected_round_trips() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a named codec variant whose type and case use raw spelling.
    let value = RawUnion::r#Named(RawPayload::new(0x1234));
    let mut bytes = [0; RawUnion::RECORD_SIZE];
    // When: explicit union selection encodes and decodes the named case.
    value.encode_variant(&mut bytes)?;
    let decoded = RawUnion::decode_variant(&bytes, RawUnionCase::r#Named)?;
    // Then: the selected payload uses its own big-endian codec.
    assert_eq!(bytes, [0x12, 0x34]);
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
fn raw_tagged_codec_when_selected_round_trips() -> Result<(), Box<dyn std::error::Error>> {
    // Given: raw names for a tag, named-codec union and variant.
    let value = RawCodecRecord::new(
        RawCodecRecordEnum::r#Named,
        RawCodecRecordUnionValues::r#Named(RawPayload::new(0x1234)),
    );
    // When: the containing record selects and encodes the named codec.
    let bytes = value.to_bytes()?;
    let decoded = RawCodecRecord::from_bytes(&bytes)?;
    // Then: the payload's byte order and selected variant are retained.
    assert_eq!(bytes, [2, 0, 0x12, 0x34]);
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
fn raw_bits_enum_when_decoded_uses_derived_enum_type() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a raw-named enum field within a raw-named bit schema.
    let value = RawEnumBitsValues {
        r#type: RawEnumBitsType::r#Busy,
    };
    // When: the typed enum is encoded and decoded.
    let raw = value.encode()?;
    let decoded = raw.decode()?;
    // Then: the discriminant occupies exactly its declared range.
    assert_eq!(raw.value(), 2);
    assert_eq!(decoded, value);
    Ok(())
}

#[test]
fn raw_named_bits_when_set_through_record_view_updates_its_slot()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: an encoded record with a raw-named reusable bits member.
    let mut bytes = [9, 7, 1, 0x34, 4];
    let value = RawBitsValues {
        r#type: false,
        r#match: 6,
    };
    // When: the derived setter writes that member through a mutable view.
    RawRecordViewMut::new(&mut bytes)?.set_mod(value)?;
    // Then: the original raw getter returns the new value at the same slot.
    assert_eq!(bytes, [9, 12, 1, 0x34, 4]);
    assert_eq!(RawRecordView::new(&bytes)?.r#mod()?, value);
    Ok(())
}
