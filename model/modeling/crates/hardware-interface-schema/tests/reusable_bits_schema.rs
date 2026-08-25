//! Named bits schemas are reusable across record and register layouts.

use hardware_interface_schema::{
    Decode, Encode, RegisterCodecError, SchemaCodecError, bits_schema, record_schema,
    register_schema,
};

bits_schema! {
    pub bits SharedFlags {
        ready: 0..1,
        mode: 8..12,
    }
}

record_schema! {
    pub record SharedFlagsRecord {
        endian: Big,
        fields: {
            flags: bits(SharedFlags),
        },
    }
}

register_schema! {
    pub register SharedFlagsRegister {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xf01,
        fields: SharedFlags,
    }
}

register_schema! {
    pub register SharedFlagsRegisterBig {
        width: 2,
        endian: Big,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xf01,
        fields: SharedFlags,
    }
}

#[test]
fn named_bits_when_embedded_in_record_and_register_reuse_one_value_type()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one semantic value from a named bits schema.
    let flags = SharedFlagsValues {
        ready: true,
        mode: 5,
    };

    // When: the same value is encoded by a record and two different registers.
    let record = SharedFlagsRecordValues { flags }.to_bytes()?;
    let mut register = [0xf0, 0xa0, 0x5a, 0xc3];
    SharedFlagsRegisterViewMut::new(&mut register)?.set_fields(flags)?;
    let mut big_register = [0xa0, 0xf0];
    SharedFlagsRegisterBigViewMut::new(&mut big_register)?.set_fields(flags)?;

    // Then: each container supplies byte order and extent while sharing bit semantics.
    assert_eq!(record, [0x05, 0x01]);
    assert_eq!(register, [0xf1, 0xa5, 0x5a, 0xc3]);
    assert_eq!(big_register, [0xa5, 0xf1]);
    assert_eq!(SharedFlagsRecordValues::from_bytes(&record)?.flags, flags);
    assert_eq!(SharedFlagsRegisterView::new(&register)?.fields()?, flags);
    assert_eq!(
        SharedFlagsRegisterBigView::new(&big_register)?.fields()?,
        flags
    );
    assert_eq!(SharedFlags::BIT_WIDTH, 12);
    assert_eq!(SharedFlags::EXTENT, 2);
    assert_eq!(SharedFlagsRegister::SPEC.fields(), &SharedFlags::FIELDS);
    assert_eq!(SharedFlagsRegisterBig::SPEC.fields(), &SharedFlags::FIELDS);
    Ok(())
}

#[test]
fn named_bits_raw_when_converted_remains_endian_neutral() -> Result<(), SchemaCodecError> {
    // Given: one normalized scalar raw value independent of any byte container.
    let raw = SharedFlagsRaw::new(0x0501);

    // When: the typed raw value is decoded and encoded through both public surfaces.
    let decoded = raw.decode()?;
    let converted = SharedFlagsValues::try_from(raw)?;
    let encoded = decoded.encode()?;
    let converted_raw = SharedFlagsRaw::try_from(converted)?;

    // Then: only the logical scalar is preserved; endian remains a container concern.
    assert_eq!(
        decoded,
        SharedFlagsValues {
            ready: true,
            mode: 5
        }
    );
    assert_eq!(decoded, converted);
    assert_eq!(encoded.value(), 0x0501);
    assert_eq!(converted_raw.value(), 0x0501);
    Ok(())
}

#[test]
fn named_bits_when_record_reserved_bits_are_nonzero_report_the_record_offset() {
    // Given: a record byte whose named bits field contains a reserved high bit.
    let bytes = [0x85, 0x01];

    // When: the record is decoded through its generated value type.
    let result = SharedFlagsRecordValues::from_bytes(&bytes);

    // Then: the shared bits decoder reports the field's record-relative offset.
    assert_eq!(
        result,
        Err(SchemaCodecError::NonZeroReserved {
            offset: 0,
            observed: 0x8000,
        })
    );
}

#[test]
fn named_bits_when_register_value_overflows_leave_bytes_unchanged() -> Result<(), RegisterCodecError>
{
    // Given: a register with nonzero declared and reserved bits.
    let mut bytes = [0x5a, 0xa5, 0xc3, 0x3c];
    let before = bytes;
    let invalid = SharedFlagsValues {
        ready: false,
        mode: 16,
    };

    // When: the reusable bits value is written through the register view.
    let result = SharedFlagsRegisterViewMut::new(&mut bytes)?.set_fields(invalid);

    // Then: narrowing fails atomically before any register byte is changed.
    assert_eq!(
        result,
        Err(RegisterCodecError::FieldValueOverflow {
            field_name: "mode",
            bit_width: 4,
            value: 16,
        })
    );
    assert_eq!(bytes, before);
    Ok(())
}
