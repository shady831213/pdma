//! Nested opaque codecs own validation of their selected representation.
use hardware_interface_schema::{SchemaCodec, SchemaCodecError, record_schema, union_schema};
record_schema! { pub record Leaf { endian: Little, fields: { value: u16 } } }
union_schema! { pub union Inner { cases: { Leaf(Leaf) }, codec: external } }
impl SchemaCodec for Inner {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::decode_variant(bytes, InnerCase::Leaf)
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_variant(bytes)
    }
}
union_schema! { pub union Middle { cases: { Inner(Inner) }, codec: external } }
impl SchemaCodec for Middle {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::decode_variant(bytes, MiddleCase::Inner)
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_variant(bytes)
    }
}
record_schema! { pub record WideLeaf { endian: Little, fields: { value: u64 } } }
union_schema! { pub union Outer { cases: { Middle(Middle), Wide(WideLeaf) } } }
#[test]
fn nested_nonzero_opaque_codecs_round_trip() {
    // Given two nested unions whose application codecs select the leaf.
    let value = Outer::Middle(Middle::Inner(Inner::Leaf(Leaf::new(0x1234))));
    let mut bytes = [0; Outer::RECORD_SIZE];
    // When encoding and explicitly decoding the outer case.
    value.encode_variant(&mut bytes).unwrap();
    // Then opaque metadata does not make valid child payload bytes reserved.
    assert_eq!(bytes, [0x34, 0x12, 0, 0, 0, 0, 0, 0]);
    assert_eq!(Outer::decode_variant(&bytes, OuterCase::Middle), Ok(value));
}

union_schema! { pub union ManualSelection { cases: { Leaf(Leaf) } } }
impl hardware_interface_schema::SchemaCodec for ManualSelection {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::decode_variant(bytes, ManualSelectionCase::Leaf)
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_variant(bytes)
    }
}
union_schema! { pub union ManualOuter { cases: { Selected(ManualSelection) } } }
#[test]
fn handwritten_union_codec_enables_nesting_without_external_opt_in() {
    // Given a union with a manually implemented selection codec.
    let value = ManualOuter::Selected(ManualSelection::Leaf(Leaf::new(17)));
    let mut bytes = [0; ManualOuter::RECORD_SIZE];
    // When using it as another union's selected child.
    value.encode_variant(&mut bytes).unwrap();
    // Then no inherent codec hook or external option is needed.
    assert_eq!(
        ManualOuter::decode_variant(&bytes, ManualOuterCase::Selected),
        Ok(value)
    );
}

#[test]
fn nested_case_tail_error_belongs_to_the_outer_union() {
    // Given a valid nested value followed by nonzero outer-only tail storage.
    let bytes = [0x34, 0x12, 1, 0, 0, 0, 0, 0];
    // When the enclosing union decodes the selected child.
    let error = Outer::decode_variant(&bytes, OuterCase::Middle).unwrap_err();
    // Then the outer union identifies its tail without blaming child padding.
    assert_eq!(
        error,
        SchemaCodecError::NonZeroReserved {
            location: hardware_interface_schema::SchemaMemberIdentity::new("Outer", "tail")
                .at_byte(2),
            observed: 1,
        }
    );
}
