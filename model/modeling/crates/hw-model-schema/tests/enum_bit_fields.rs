//! Shared enum bit fields retain typed decoding and raw register storage.
use hw_model_schema::{
    Decode, Encode, RegisterCodecError, RegisterLayout, SchemaCodecError, SchemaCoordinate,
    bits_schema, register_schema,
};

bits_schema! {
    pub bits FaultBits(u16) {
        code: 4..6 => enum { variants: { None = 0, Read = 1, Write = 2 } },
        enabled: 0..1,
    }
}

register_schema! {
    pub register Fault {
        width: 2, endian: Big, access: ReadWrite,
        reset: Constant(0x20), writable_mask: 0x30,
        fields: { code: 4..6 => enum { variants: { None = 0, Read = 1, Write = 2 } } },
    }
}

register_schema! {
    pub register SharedFault {
        width: 2, endian: Little, access: ReadWrite,
        reset: Constant(0x20), writable_mask: 0x31,
        fields: FaultBits,
    }
}

#[test]
fn unknown_code_when_decoded_reports_bit_identity() {
    // Given: the unassigned code 3 in the declared enum range.
    let raw = FaultBitsRaw::new(0x30);
    // When: typed decoding is requested.
    let result = raw.decode();
    // Then: the failure identifies the enum member and local bit.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: FaultBits::CODE_IDENTITY.at_bit(4),
            observed: 3,
        })
    );
}

#[test]
fn register_enum_when_updated_preserves_other_bits_and_reset()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a nonzero reset and undeclared register bits.
    let raw = FaultValues::new(0xaa3f);
    // When: a typed enum value replaces the unknown code.
    let updated = raw.with_code(FaultCode::Read);
    // Then: reserved bits survive and default uses the actual reset code.
    assert_eq!(updated.raw(), 0xaa1f);
    assert_eq!(FaultValues::default().code()?, FaultCode::Write);
    assert_eq!(updated.encode()?.as_bytes(), &[0xaa, 0x1f]);
    assert_eq!(
        raw.code(),
        Err(RegisterCodecError::UnknownTag {
            location: Fault::CODE_IDENTITY.at_bit(4),
            observed: 3,
        })
    );
    Ok(())
}

#[test]
fn named_enum_when_embedded_decodes_the_register_reset() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a register embedding a reusable enum bit layout.
    let reset = SharedFaultValues::default();
    // When: the typed fields are read.
    let fields = reset.fields()?;
    // Then: the reset code selects the declared nonzero variant.
    assert_eq!(fields.code(), FaultBitsCode::Write);
    Ok(())
}

#[test]
fn enum_register_view_when_rebased_reports_absolute_bit() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: unknown code 3 in a big-endian register placed at byte 8.
    let bytes = [0, 0x30];
    let view = Fault::view_at(&bytes, SchemaCoordinate::Byte(8))?;
    // When: its enum member is read.
    let result = view.code();
    // Then: the error refers to bit 68 in the containing layout.
    assert_eq!(
        result,
        Err(RegisterCodecError::UnknownTag {
            location: Fault::CODE_IDENTITY.at_bit(68),
            observed: 3,
        })
    );
    Ok(())
}

#[test]
fn named_enum_register_view_when_rebased_reports_absolute_bit()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: unknown code 3 in a named layout placed at byte 8.
    let bytes = [0x30, 0];
    let view = SharedFault::view_at(&bytes, SchemaCoordinate::Byte(8))?;
    // When: its shared fields are decoded.
    let result = view.fields();
    // Then: the shared member reports bit 68 in the containing layout.
    assert_eq!(
        result,
        Err(RegisterCodecError::UnknownTag {
            location: FaultBits::CODE_IDENTITY.at_bit(68),
            observed: 3,
        })
    );
    Ok(())
}

#[test]
fn enum_register_view_when_written_preserves_reserved_bits()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a big-endian register containing unknown enum and reserved bits.
    let mut bytes = [0xab, 0xff];
    // When: the enum field is written through its borrowed view.
    FaultViewMut::new(&mut bytes)?.set_code(FaultCode::Write)?;
    // Then: only the declared field bits change.
    assert_eq!(bytes, [0xab, 0xef]);
    Ok(())
}
