use hardware_interface_schema::{
    Decode, Encode, SchemaCodec, SchemaCodecError, record_schema, union_schema,
};

record_schema! { record Byte { endian: Little, fields: { value: u8 } } }
union_schema! { union Transformed { cases: { Byte(Byte) }, codec: external } }

impl SchemaCodec for Transformed {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        let Self::Byte(mut value) = Self::decode_variant(bytes, TransformedCase::Byte)?;
        value.value ^= 0xa5;
        Ok(Self::Byte(value))
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        let Self::Byte(value) = *value;
        Self::Byte(Byte::new(value.value ^ 0xa5)).encode_variant(bytes)
    }
}

#[test]
fn generated_decoders_apply_the_trait_transformation_exactly_once() {
    // Given a stored byte different from its application-level value.
    let bytes = [0x96];
    let expected = Transformed::Byte(Byte::new(0x33));
    let raw = TransformedRaw::new(bytes);
    let mut mutable = bytes;
    // When decoding through the trait and each generated adapter.
    let results = [
        <Transformed as SchemaCodec>::decode(&bytes),
        Transformed::from_bytes(&bytes),
        raw.decode(),
        Transformed::try_from(raw),
        TransformedView::new(&bytes).unwrap().values(),
        TransformedView::new(&bytes).unwrap().decode(),
        TransformedViewMut::new(&mut mutable).unwrap().values(),
    ];
    // Then every path applies exactly one transformation.
    assert_eq!(results, [Ok(expected); 7]);
    assert_eq!(
        TransformedView::new(&bytes)
            .unwrap()
            .variant(TransformedCase::Byte),
        Ok(Transformed::Byte(Byte::new(0x96)))
    );
}

#[test]
fn generated_encoders_apply_the_trait_transformation_exactly_once() {
    // Given an application value whose wire representation is different.
    let value = Transformed::Byte(Byte::new(0x33));
    let mut trait_bytes = [0xcc; 2];
    let mut inherent_bytes = [0xcc; 2];
    let mut view_bytes = [0xcc; 2];
    // When encoding through the trait and every generated adapter.
    <Transformed as SchemaCodec>::encode(&mut trait_bytes, &value).unwrap();
    value.encode_into(&mut inherent_bytes).unwrap();
    TransformedViewMut::new(&mut view_bytes)
        .unwrap()
        .set_values(&value)
        .unwrap();
    let owned = [
        value.to_bytes().unwrap(),
        value.encode().unwrap().into_bytes(),
        TransformedRaw::try_from(value).unwrap().into_bytes(),
    ];
    // Then each path transforms once and preserves bytes beyond its extent.
    assert_eq!(owned, [[0x96]; 3]);
    assert_eq!([trait_bytes, inherent_bytes, view_bytes], [[0x96, 0xcc]; 3]);
    TransformedViewMut::new(&mut view_bytes)
        .unwrap()
        .set_variant(&value)
        .unwrap();
    assert_eq!(view_bytes, [0x33, 0xcc]);
}
