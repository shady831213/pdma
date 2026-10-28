//! Application codecs own policy for opted-in union adapters.
use hw_model_schema::{
    Decode, Encode, SchemaCodec, SchemaCodecError, SchemaLayout, record_schema, union_schema,
};

mod dispatch;

record_schema! { pub record Small { endian: Big, fields: { value: u16 } } }
record_schema! { pub record Large { endian: Little, fields: { value: u64 } } }
union_schema! { pub union Fixed { cases: { Small(Small), Large(Large) }, codec: external, } }
union_schema! { pub union Checked { cases: { Small(Small), Large(Large) }, codec: external } }

// Application trait implementations can live outside the declaration module.
mod application {
    use super::*;

    impl SchemaCodec for Fixed {
        fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
            Self::decode_variant(bytes, FixedCase::Small)
        }
        fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
            value.encode_variant(bytes)
        }
    }

    impl SchemaCodec for Checked {
        fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
            let value = Self::decode_variant(bytes, CheckedCase::Small)?;
            validate(&value)?;
            Ok(value)
        }
        fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
            validate(value)?;
            value.encode_variant(bytes)
        }
    }

    fn validate(value: &Checked) -> Result<(), SchemaCodecError> {
        let observed = match value {
            Checked::Small(value) => u64::from(value.value),
            Checked::Large(value) => value.value,
        };
        if observed == 7 {
            Ok(())
        } else {
            Err(policy_error(observed))
        }
    }
}

const fn policy_error(observed: u64) -> SchemaCodecError {
    use hw_model_schema::{SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};
    SchemaCodecError::ConstantMismatch {
        location: SchemaLocation::new(
            SchemaMemberIdentity::new("application", "value"),
            SchemaCoordinate::Byte(0),
        ),
        observed,
        expected: 7,
    }
}

#[test]
fn external_fixed_case_needs_no_selector_and_keeps_natural_geometry() {
    // Given a codec that always selects Small and no selector field or method.
    let value = Fixed::Small(Small::new(0x1234));
    let expected = [0x12, 0x34, 0, 0, 0, 0, 0, 0];
    // When using each mechanical owned conversion.
    let raw = value.encode().unwrap();
    // Then bytes, geometry, and decoded values follow the application codec.
    assert_eq!(raw.into_bytes(), expected);
    assert_eq!(value.to_bytes(), Ok(expected));
    assert_eq!(raw.decode(), Ok(value));
    assert_eq!(Fixed::try_from(raw), Ok(value));
    assert_eq!(FixedRaw::try_from(value), Ok(raw));
    assert_eq!(<Fixed as SchemaCodec>::decode(&expected), Ok(value));
    assert_eq!(<Fixed as SchemaLayout>::EXTENT, 8);
    assert_eq!(<Fixed as SchemaLayout>::ALIGNMENT, 8);
    assert_eq!(
        <Fixed as SchemaLayout>::view(&expected).unwrap().decode(),
        Ok(value)
    );
    let mut bytes = [0xff; 10];
    <Fixed as SchemaLayout>::view_mut(&mut bytes)
        .unwrap()
        .set_values(&value)
        .unwrap();
    assert_eq!(&bytes[..8], &expected);
    assert_eq!(&bytes[8..], &[0xff; 2]);
}

#[test]
fn normal_views_delegate_application_validation_and_explicit_views_remain_available() {
    // Given valid raw union storage that violates application policy.
    let bytes = [0, 8, 0, 0, 0, 0, 0, 0];
    let mut mutable = bytes;
    let explicit = Checked::Small(Small::new(8));
    let error = policy_error(8);
    // When decoding through all generated policy-facing paths.
    // Then they preserve the exact application error, while explicit interpretation bypasses policy.
    assert_eq!(CheckedRaw::new(bytes).decode(), Err(error));
    assert_eq!(Checked::from_bytes(&bytes), Err(error));
    assert_eq!(Checked::try_from(CheckedRaw::new(bytes)), Err(error));
    assert_eq!(<Checked as SchemaCodec>::decode(&bytes), Err(error));
    assert_eq!(CheckedView::new(&bytes).unwrap().values(), Err(error));
    assert_eq!(CheckedView::new(&bytes).unwrap().decode(), Err(error));
    assert_eq!(
        CheckedViewMut::new(&mut mutable).unwrap().values(),
        Err(error)
    );
    assert_eq!(
        CheckedView::new(&bytes)
            .unwrap()
            .variant(CheckedCase::Small),
        Ok(explicit)
    );
    CheckedViewMut::new(&mut mutable)
        .unwrap()
        .set_variant(&explicit)
        .unwrap();
    assert_eq!(mutable, bytes);
}

#[test]
fn encoding_errors_and_atomicity_come_from_the_application_trait() {
    // Given an application-invalid value and preexisting destination bytes.
    let invalid = Checked::Small(Small::new(8));
    let mut bytes = [0xaa; 10];
    let error = policy_error(8);
    // When all generated encoders delegate to the validating application trait.
    assert_eq!(invalid.encode(), Err(error));
    assert_eq!(invalid.to_bytes(), Err(error));
    assert_eq!(invalid.encode_into(&mut bytes), Err(error));
    assert_eq!(CheckedRaw::try_from(invalid), Err(error));
    assert_eq!(
        <Checked as SchemaCodec>::encode(&mut bytes, &invalid),
        Err(error)
    );
    assert_eq!(
        CheckedViewMut::new(&mut bytes)
            .unwrap()
            .set_values(&invalid),
        Err(error)
    );
    // Then the trait's rejection leaves the destination unchanged.
    assert_eq!(bytes, [0xaa; 10]);
    let valid = Checked::Small(Small::new(7));
    let mut view = CheckedViewMut::new(&mut bytes).unwrap();
    view.set_values(&valid).unwrap();
    assert_eq!(view.values(), Ok(valid));
    assert_eq!(bytes, [0, 7, 0, 0, 0, 0, 0, 0, 0xaa, 0xaa]);
}

#[test]
fn short_buffers_and_reserved_bytes_keep_existing_errors() {
    // Given short storage and invalid inactive bytes.
    let mut short = [0xaa; 7];
    let short_error = SchemaCodecError::BufferTooShort {
        required: 8,
        actual: 7,
    };
    // When views check extent and the external decoder uses explicit union validation.
    assert_eq!(FixedView::new(&short).unwrap_err(), short_error);
    assert_eq!(FixedViewMut::new(&mut short).unwrap_err(), short_error);
    assert_eq!(Fixed::ZERO.encode_into(&mut short), Err(short_error));
    assert_eq!(Fixed::from_bytes(&short), Err(short_error));
    assert_eq!(short, [0xaa; 7]);
    let bytes = [0, 7, 1, 0, 0, 0, 0, 0];
    // Then padding errors propagate unchanged through every decoder entrypoint.
    let expected = Checked::decode_variant(&bytes, CheckedCase::Small).unwrap_err();
    assert!(matches!(expected, SchemaCodecError::NonZeroReserved { .. }));
    assert_eq!(CheckedRaw::new(bytes).decode(), Err(expected));
    assert_eq!(CheckedView::new(&bytes).unwrap().values(), Err(expected));
}
