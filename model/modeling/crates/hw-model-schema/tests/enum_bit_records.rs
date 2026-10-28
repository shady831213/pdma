//! Record enum fields and bit fields share discriminants and metadata.
use hw_model_schema::{SchemaCodecError, bits_schema, record_schema};

bits_schema! {
    pub bits StateBits(u16) {
        code: 7..10 => enum { variants: {
            #[short_description = "Idle"]
            #[description = "No active transfer."]
            Idle = 0, Busy = 5,
        } },
    }
}

record_schema! {
    pub record States {
        endian: Big,
        fields: {
            tag: enum(u16) { variants: {
                #[short_description = "Idle"]
                #[description = "No active transfer."]
                Idle = 0, Busy = 5,
            } },
            shared: bits(StateBits),
            inline: bits(u16) { fields: {
                code: 7..10 => enum { variants: { Idle = 0, Busy = 5 } },
            } },
        },
    }
}

#[test]
fn enum_bits_when_record_encoded_cross_bytes_in_big_endian() -> Result<(), SchemaCodecError> {
    // Given: equivalent scalar, shared-bit and inline-bit enum variants.
    let value = States {
        tag: StatesTag::Busy,
        shared: StateBitsValues::new(StateBitsCode::Busy),
        inline: StatesInlineValues {
            code: StatesInlineValuesCode::Busy,
        },
    };
    // When: the record crosses its byte encoding boundary.
    let bytes = value.to_bytes()?;
    // Then: exact big-endian bytes and enum types survive the round trip.
    assert_eq!(bytes, [0, 5, 2, 0x80, 2, 0x80]);
    assert_eq!(States::from_bytes(&bytes)?, value);
    Ok(())
}

#[test]
fn unknown_named_enum_when_embedded_reports_absolute_bit() {
    // Given: code 7 at a named bits field's nonzero record offset.
    let bytes = [0, 0, 3, 0x80, 0, 0];
    // When: the record is decoded.
    let result = States::from_bytes(&bytes);
    // Then: the shared member identity retains the root-relative bit position.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: StateBits::CODE_IDENTITY.at_bit(23),
            observed: 7,
        })
    );
}

#[test]
fn unknown_inline_enum_when_embedded_reports_absolute_bit() {
    // Given: code 7 at an inline bits field's nonzero record offset.
    let bytes = [0, 0, 0, 0, 3, 0x80];
    // When: the record is decoded.
    let result = States::from_bytes(&bytes);
    // Then: the inline member identity retains the root-relative bit position.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: StatesInlineValues::CODE_IDENTITY.at_bit(39),
            observed: 7,
        })
    );
}

#[test]
fn enums_when_introspected_expose_codes_and_variant_documentation() {
    // Given: record, named-bit and inline-bit enum declarations.
    let tag = States::FIELDS[0].enum_definition().expect("tag enum");
    let shared = States::FIELDS[1]
        .bits()
        .first()
        .expect("shared bit")
        .definition()
        .enum_definition()
        .expect("shared enum");
    let inline = States::FIELDS[2]
        .bits()
        .first()
        .expect("inline bit")
        .definition()
        .enum_definition()
        .expect("inline enum");
    // When: metadata is inspected through the public schema surface.
    let variants = tag.variants();
    // Then: codes and documentation are shared without losing inline definitions.
    assert_eq!(tag, shared);
    assert_eq!(variants.get(1).expect("busy").value(), 5);
    assert_eq!(
        variants
            .first()
            .expect("idle")
            .documentation()
            .short_description,
        Some("Idle")
    );
    assert_eq!(
        variants.first().expect("idle").documentation().description,
        Some("No active transfer.")
    );
    assert_eq!(inline.variants().get(1).expect("inline busy").value(), 5);
}
