//! Decoder locals and discriminator order are independent of wire field names.
use hw_model_schema::record_schema;

record_schema! {
    pub record Names {
        endian: Little,
        fields: { bytes: u16, actual: u16, field: u16, end: u16, r#type: u16 },
    }
}
record_schema! {
    pub record LaterTag {
        endian: Little,
        fields: {
            payload: tagged_union { tag: self.kind, cases: { Number { number: u16 } } },
            kind: enum(u8) { variants: { Number = 1 } },
        },
    }
}
#[test]
fn decoder_names_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let value = Names::new(1, 2, 3, 4, 5);
    let bytes = value.to_bytes()?;
    assert_eq!(bytes, [1, 0, 2, 0, 3, 0, 4, 0, 5, 0]);
    assert_eq!(Names::from_bytes(&bytes)?, value);
    Ok(())
}
#[test]
fn later_discriminator_preserves_wire_order() -> Result<(), Box<dyn std::error::Error>> {
    let value = LaterTag::new(
        LaterTagPayloadValues::Number(LaterTagPayloadNumberValues { number: 0x1234 }),
        LaterTagKind::Number,
    );
    let bytes = value.to_bytes()?;
    assert_eq!(&bytes[..3], &[0x34, 0x12, 1]);
    assert_eq!(LaterTag::from_bytes(&bytes)?, value);
    Ok(())
}
