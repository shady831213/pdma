//! Register bit ranges use the shared record bit-field language.

use hardware_interface_schema::{Decode, Encode, RegisterCodecError, register_schema};

register_schema! {
    pub register PackedControl {
        width: 2,
        alignment: 2,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0x15),
        writable_mask: 0xf,
        fields: {
            enable: 0..1,
            mode: 1..4,
            count: 8..16,
        },
    }
}

register_schema! {
    pub register BigPacked {
        width: 2,
        endian: Big,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xf,
        fields: {
            mode: 0..4,
        },
    }
}

#[test]
fn typed_ranges_when_read_and_written_preserve_logical_bits_and_endian()
-> Result<(), RegisterCodecError> {
    let fields = PackedControl::SPEC.fields();
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.id(), field.lsb(), field.bit_width()))
            .collect::<Vec<_>>(),
        vec![(1, 0, 1), (2, 1, 3), (3, 8, 8)]
    );

    let mut little = [0x15, 0xab];
    let mut view = PackedControlViewMut::new(&mut little)?;
    assert!(view.enable()?);
    assert_eq!(view.mode()?, 2);
    assert_eq!(view.count()?, 0xab);
    view.set_enable(false)?;
    view.set_mode(7)?;
    view.set_count(0x34)?;
    assert_eq!(little, [0x1e, 0x34]);

    let mut big = [0_u8; 2];
    BigPackedViewMut::new(&mut big)?.set_mode(0xa)?;
    assert_eq!(big, [0, 0x0a]);
    assert_eq!(BigPackedView::new(&big)?.mode()?, 0xa);
    Ok(())
}

#[test]
fn narrow_range_when_value_overflows_preserves_prior_bytes() -> Result<(), RegisterCodecError> {
    let mut bytes = [0x5a, 0xa5];
    let before = bytes;
    let error = PackedControlViewMut::new(&mut bytes)?.set_mode(8);
    assert_eq!(
        error,
        Err(RegisterCodecError::FieldValueOverflow {
            field_name: "mode",
            bit_width: 3,
            value: 8,
        })
    );
    assert_eq!(bytes, before);
    Ok(())
}

#[test]
fn register_values_when_defaulted_use_reset_and_preserve_undeclared_bits()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a nonzero reset register and a full scalar containing undeclared bits.
    let reset = PackedControlValues::default();
    let reset_raw = PackedControlRaw::default();
    let current = PackedControlValues::new(0xab15);

    // When: typed fields are replaced and both codec directions are exercised.
    let updated = current.with_mode(7)?;
    let encoded = reset.encode()?;
    let decoded = reset_raw.decode()?;
    let converted = PackedControlValues::try_from(reset_raw)?;
    let converted_raw = PackedControlRaw::try_from(reset)?;

    // Then: default means reset, endian belongs to the register, and other bits survive.
    assert_eq!(reset.raw(), 0x15);
    assert_eq!(reset_raw.as_bytes(), &[0x15, 0]);
    assert_eq!(updated.raw(), 0xab1f);
    assert_eq!(updated.mode(), 7);
    assert_eq!(encoded, reset_raw);
    assert_eq!(decoded, reset);
    assert_eq!(converted, reset);
    assert_eq!(converted_raw, reset_raw);
    Ok(())
}
