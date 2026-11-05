//! Composition requires only the shared codec contract, never inherent APIs.
use hw_model_schema::{SchemaCodec, SchemaCodecError, SchemaLayout, record_schema, union_schema};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Handwritten semantic value with no inherent schema API.
pub struct Manual(#[doc = "Encoded scalar."] pub u16);
impl SchemaLayout for Manual {
    type Value = Self;
    type View<'a> = &'a [u8];
    type ViewMut<'a> = &'a mut [u8];
    const NAME: &'static str = "Manual";
    const ALIGNMENT: usize = 2;
    const EXTENT: usize = 2;
    const ZERO: Self = Self(0);

    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> {
        bytes.get(..2).ok_or(SchemaCodecError::BufferTooShort {
            required: 2,
            actual: bytes.len(),
        })
    }
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        let actual = bytes.len();
        bytes.get_mut(..2).ok_or(SchemaCodecError::BufferTooShort {
            required: 2,
            actual,
        })
    }
}
impl SchemaCodec for Manual {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        let [low, high] = Self::view(bytes)? else {
            return Err(SchemaCodecError::BufferTooShort {
                required: 2,
                actual: bytes.len(),
            });
        };
        Ok(Self(u16::from_le_bytes([*low, *high])))
    }
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        Self::view_mut(bytes)?.copy_from_slice(&value.0.to_le_bytes());
        Ok(())
    }
}
union_schema! { pub union Choice { cases: { Manual(Manual) } } }
record_schema! { pub record Container { endian: Little, fields: { prefix: u8, child: Manual } } }
#[test]
fn handwritten_struct_nests_without_inherent_schema_members() {
    // Given a handwritten codec with only trait members.
    let value = Choice::Manual(Manual(0x1234));
    let mut bytes = [0; Choice::RECORD_SIZE];
    // When the generated union encodes and selects that codec.
    value.encode_variant(&mut bytes).unwrap();
    // Then its semantic value and record-field composition round trip.
    assert_eq!(
        Choice::decode_variant(&bytes, ChoiceCase::Manual),
        Ok(value)
    );
    let parent = Container::new(9, Manual(0x1234));
    assert_eq!(
        Container::from_bytes(&parent.to_bytes().unwrap()),
        Ok(parent)
    );
}

/// Codec marker with a scalar associated value.
#[derive(Debug)]
pub enum CheckedCodec {}
impl SchemaLayout for CheckedCodec {
    type Value = u16;
    type View<'a> = &'a [u8];
    type ViewMut<'a> = &'a mut [u8];
    const NAME: &'static str = "CheckedCodec";
    const ALIGNMENT: usize = 2;
    const EXTENT: usize = 2;
    const ZERO: u16 = 0;

    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> {
        <Manual as SchemaLayout>::view(bytes)
    }
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        <Manual as SchemaLayout>::view_mut(bytes)
    }
}
impl SchemaCodec for CheckedCodec {
    fn decode(bytes: &[u8]) -> Result<u16, SchemaCodecError> {
        let value = <Manual as SchemaCodec>::decode(bytes)?.0;
        check(value)?;
        Ok(value)
    }
    fn encode(bytes: &mut [u8], value: &u16) -> Result<(), SchemaCodecError> {
        <Manual as SchemaCodec>::encode(bytes, &Manual(*value))?;
        check(*value)
    }
}
fn check(value: u16) -> Result<(), SchemaCodecError> {
    if value == u16::MAX {
        Err(SchemaCodecError::ConstantMismatch {
            location: hw_model_schema::SchemaMemberIdentity::new("CheckedCodec", "value")
                .at_byte(0),
            observed: u64::from(value),
            expected: 7,
        })
    } else {
        Ok(())
    }
}
record_schema! { pub record Wide { endian: Little, fields: { value: u64 } } }
union_schema! { pub union Mixed { cases: { Manual(Manual), Checked(CheckedCodec), Wide(Wide) } } }
record_schema! { pub record WithMarker { endian: Little, fields: { prefix: u8, child: CheckedCodec } } }
record_schema! { pub record Tagged { endian: Little, fields: {
    kind: enum(u8) { variants: { Manual = 0, Checked = 1, Wide = 2 } },
    payload: tagged_union { tag: self.kind, cases: { Manual(Manual), Checked(CheckedCodec), Wide(Wide) } },
} } }
#[test]
fn codec_marker_value_is_independent_of_implementor_type() {
    // Given an enum marker whose decoded value is a scalar.
    let value = Mixed::Checked(0x1234);
    let mut bytes = [0; Mixed::RECORD_SIZE];
    // When composing it in an untagged union, record field and tagged union.
    value.encode_variant(&mut bytes).unwrap();
    let parent = WithMarker::new(9, 0x1234);
    let tagged = Tagged::new(TaggedKind::Manual, TaggedPayloadValues::Checked(0x1234));
    // Then each generated container uses the associated value and natural layout.
    assert_eq!(Mixed::decode_variant(&bytes, MixedCase::Checked), Ok(value));
    assert_eq!(
        WithMarker::from_bytes(&parent.to_bytes().unwrap()),
        Ok(parent)
    );
    assert_eq!(Tagged::from_bytes(&tagged.to_bytes().unwrap()), Ok(tagged));
    assert_eq!(WithMarker::CHILD_OFFSET, 2);
    assert_eq!(Mixed::RECORD_SIZE, 8);
    bytes[2] = 1;
    assert_eq!(
        Mixed::decode_variant(&bytes, MixedCase::Checked),
        Err(SchemaCodecError::NonZeroReserved {
            location: hw_model_schema::SchemaMemberIdentity::new("Mixed", "tail").at_byte(2),
            observed: 1,
        })
    );
}
#[test]
fn child_failure_rebases_and_parent_encodes_atomically() {
    // Given a codec that writes before reporting a rejected semantic value.
    let invalid = WithMarker::new(9, u16::MAX);
    let mut bytes = [0xaa; WithMarker::RECORD_SIZE + 2];
    let expected = check(u16::MAX)
        .unwrap_err()
        .rebased(hw_model_schema::SchemaCoordinate::Byte(2));
    // When encoding through a generated parent, or decoding at its child offset.
    assert_eq!(invalid.encode_into(&mut bytes), Err(expected));
    assert_eq!(WithMarker::from_bytes(&[9, 0, 0xff, 0xff]), Err(expected));
    // Then errors preserve identity and coordinates, and parent storage is unchanged.
    assert_eq!(bytes, [0xaa; WithMarker::RECORD_SIZE + 2]);
    let mut union_bytes = [0xaa; Mixed::RECORD_SIZE];
    assert_eq!(
        Mixed::Checked(u16::MAX).encode_variant(&mut union_bytes),
        check(u16::MAX)
    );
    assert_eq!(union_bytes, [0xaa; Mixed::RECORD_SIZE]);
    let tagged = Tagged::new(TaggedKind::Manual, TaggedPayloadValues::Checked(u16::MAX));
    let mut tagged_bytes = [0xaa; Tagged::RECORD_SIZE];
    assert!(tagged.encode_into(&mut tagged_bytes).is_err());
    assert_eq!(tagged_bytes, [0xaa; Tagged::RECORD_SIZE]);
}
#[test]
fn trait_only_field_views_and_opaque_metadata_compose() {
    // Given a record containing a codec with no structural metadata override.
    let mut bytes = WithMarker::new(9, 7).to_bytes().unwrap();
    // When using generated child views and resolving parent metadata.
    assert_eq!(
        WithMarkerView::new(&bytes).unwrap().child().unwrap(),
        &[7, 0]
    );
    WithMarkerViewMut::new(&mut bytes)
        .unwrap()
        .child_mut()
        .unwrap()
        .copy_from_slice(&[8, 0]);
    let spec = <CheckedCodec as SchemaLayout>::SPEC;
    // Then opaque storage retains its size/name, without pretending to describe fields.
    assert_eq!(WithMarker::from_bytes(&bytes).unwrap().child, 8);
    assert_eq!(spec.name(), "CheckedCodec");
    assert_eq!(spec.size(), 2);
    assert!(spec.fields().is_empty());
    assert!(WithMarker::SPEC.resolve().is_ok());
}

record_schema! { pub record Sized { endian: Little, fields: {
    size: u16 { const: CheckedCodec::record_size }, child: CheckedCodec,
} } }
#[test]
fn derived_size_uses_the_codec_extent() {
    // Given an external marker with no inherent size constant.
    let value = Sized::new(7);
    // When a generated scalar derives the child's size.
    let bytes = value.to_bytes().unwrap();
    // Then only the shared trait extent is needed.
    assert_eq!(bytes, [2, 0, 7, 0]);
    assert_eq!(Sized::from_bytes(&bytes), Ok(value));
}

#[cfg(feature = "export-c-header")]
#[test]
fn opaque_field_metadata_exports_without_describing_internals() {
    use hw_model_schema::analysis::AnalyzedSchema;
    use hw_model_schema::export::InMemoryArtifactSink;
    use hw_model_schema::export::backends::c_header::CRecordBackend;
    // Given a parent containing trait-default opaque metadata.
    let mut sink = InMemoryArtifactSink::new();
    // When exporting its validated layout.
    let schema = AnalyzedSchema::opaque("opaque.h", WithMarker::SPEC).unwrap();
    let id = schema.view("opaque.h").unwrap().id.clone();
    hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)
        .unwrap()
        .generate(
            |data| CRecordBackend::new(data).with_name(id, "opaque"),
            &mut sink,
        )
        .unwrap();
    let artifact = sink.into_artifacts().remove(0);
    let header = String::from_utf8(artifact.contents).unwrap();
    // Then the complete parent size and known prefix survive the export.
    assert!(header.contains("#define C_OPAQQUE_SIZE UINT64_C(4)"));
    assert!(header.contains("C_PREFIX_OFFSET UINT64_C(0)"));
    assert!(header.contains("RECORD_C_CHILD_SIZE UINT64_C(2)"));
}
