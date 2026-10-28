//! PIN coverage for the current flat, nested, and repeated register views.

use hw_model_schema::{
    Decode, Encode, RegisterLocation, RegisterValueError, SchemaCodecError, register_map,
    register_schema,
};

register_schema! {
    pub register PinControl {
        width: 2,
        alignment: 2,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0x1203),
        writable_mask: 0xff0f,
        fields: {
            enable: 0..1,
            mode: 1..4,
            tag: 8..16,
        },
    }
}

register_schema! {
    pub register PinCounter {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadOnly,
        reset: PostReset(0xa1b2_c3d4),
        writable_mask: 0,
        fields: {},
    }
}

register_map! {
    pub map PinLane {
        alignment: 4,
        placements: [
            control: PinControl,
            counter: PinCounter,
        ],
    }
}

register_map! {
    pub map PinDevice {
        alignment: 8,
        extent: 48,
        placements: [
            header: PinControl,
            lanes: PinLane[2],
            tail: PinControl { offset: 40 },
        ],
    }
}

#[test]
fn flat_values_raw_views_preserve_literal_fields() -> Result<(), Box<dyn std::error::Error>> {
    // Given: one little-endian register value with typed fields and undeclared bits.
    let value = PinControlValues::new(0xabcd);

    // When: the owned, raw, immutable, and mutable flat-register surfaces are exercised.
    let raw = value.encode()?;
    let decoded = PinControlRaw::new([0xcd, 0xab]).decode()?;
    let view = PinControlView::new(raw.as_bytes())?;
    let mut changed = raw.into_bytes();
    PinControlViewMut::new(&mut changed)?.set_mode(0)?;

    // Then: bytes, typed fields, and field-preserving mutation are exact.
    assert_eq!(raw.as_bytes(), &[0xcd, 0xab]);
    assert_eq!(decoded, value);
    assert_eq!(view.value()?, 0xabcd);
    assert!(view.enable()?);
    assert_eq!(view.mode()?, 6);
    assert_eq!(view.tag()?, 0xab);
    assert_eq!(changed, [0xc1, 0xab]);
    let changed_view = PinControlView::new(&changed)?;
    assert!(changed_view.enable()?);
    assert_eq!(changed_view.mode()?, 0);
    assert_eq!(changed_view.tag()?, 0xab);
    println!("flat raw=[cd, ab] changed=[c1, ab] fields=(true, 6, 171)");
    Ok(())
}

#[test]
fn nested_repeated_values_views_lookup_and_padding_are_literal()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: nested lanes repeated twice, with explicit tail placement and reset defaults.
    let values = PinDeviceValues {
        header: PinControlValues::new(0x1123),
        lanes: [
            PinLaneValues {
                control: PinControlValues::new(0x2245),
                counter: PinCounterValues::new(0x0102_0304),
            },
            PinLaneValues {
                control: PinControlValues::new(0x5667),
                counter: PinCounterValues::new(0x0a0b_0c0d),
            },
        ],
        tail: PinControlValues::new(0x7889),
    };

    // When: the map is encoded, decoded, viewed, and resolved at exact starts.
    let raw = values.encode()?;
    let round_trip = PinDeviceRaw::new(*raw.as_bytes()).decode()?;
    let view = PinDeviceView::new(raw.as_bytes())?;
    let lane = view.lanes(1)?;
    let lane_counter = lane.counter()?;
    let mut changed = raw.into_bytes();
    PinDeviceViewMut::new(&mut changed)?
        .lanes_mut(1)?
        .control_mut()?
        .set_mode(7)?;
    let resolved = PinDevice::resolve()?;
    let located = PinDevice::locate(12);

    // Then: all leaf bytes and all inter-placement padding retain the current layout.
    assert_eq!(&raw.as_bytes()[0..2], &[0x23, 0x11]);
    assert_eq!(&raw.as_bytes()[4..6], &[0x45, 0x22]);
    assert_eq!(&raw.as_bytes()[8..12], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(&raw.as_bytes()[12..14], &[0x67, 0x56]);
    assert_eq!(&raw.as_bytes()[16..20], &[0x0d, 0x0c, 0x0b, 0x0a]);
    assert_eq!(&raw.as_bytes()[40..42], &[0x89, 0x78]);
    assert!(raw.as_bytes()[2..4].iter().all(|byte| *byte == 0));
    assert!(raw.as_bytes()[20..40].iter().all(|byte| *byte == 0));
    assert!(raw.as_bytes()[42..48].iter().all(|byte| *byte == 0));
    assert_eq!(round_trip, values);
    assert_eq!(view.header()?.value()?, 0x1123);
    assert_eq!(lane_counter.value()?, 0x0a0b_0c0d);
    assert_eq!(&changed[12..14], &[0x6f, 0x56]);
    let defaults = PinDeviceValues::default();
    let default_raw = defaults.encode()?;
    assert_eq!(defaults.header.raw(), 0x1203);
    assert_eq!(defaults.lanes[0].counter.raw(), 0xa1b2_c3d4);
    assert_eq!(defaults.tail.raw(), 0x1203);
    assert_eq!(&default_raw.as_bytes()[0..2], &[0x03, 0x12]);
    assert_eq!(&default_raw.as_bytes()[4..6], &[0x03, 0x12]);
    assert_eq!(&default_raw.as_bytes()[8..12], &[0xd4, 0xc3, 0xb2, 0xa1]);
    assert_eq!(&default_raw.as_bytes()[40..42], &[0x03, 0x12]);
    assert_eq!(
        resolved
            .registers()
            .iter()
            .map(|register| (register.path(), register.offset()))
            .collect::<Vec<_>>(),
        [
            ("PinDevice.header", 0),
            ("PinDevice.lanes[0].control", 4),
            ("PinDevice.lanes[0].counter", 8),
            ("PinDevice.lanes[1].control", 12),
            ("PinDevice.lanes[1].counter", 16),
            ("PinDevice.tail", 40),
        ]
    );
    assert!(matches!(
        located,
        Some(RegisterLocation::Repeated {
            bank_name: "lanes",
            instance: 1,
            offset: 12,
            register,
        }) if register.name() == "CONTROL" && register.width() == 2
    ));
    assert!(PinDevice::locate(13).is_none());
    assert!(PinDevice::locate(41).is_none());
    println!(
        "nested offsets=0:23_11,4:45_22,8:04_03_02_01,12:67_56,16:0d_0c_0b_0a,40:89_78 padding=zero changed12=6f_56"
    );
    Ok(())
}

#[test]
fn adverse_inputs_reject_before_mutating_bytes() -> Result<(), Box<dyn std::error::Error>> {
    // Given: short, valid-sized, and flat register buffers with sentinel bytes.
    let mut short = [0xa5_u8; 47];
    let short_before = short;
    let mut indexed = [0x5a_u8; 48];
    let indexed_before = indexed;
    let mut field = [0x34_u8, 0x12];
    let field_before = field;

    // When: construction, repeated-index, and overflowing-field operations are attempted.
    let short_result = PinDeviceViewMut::new(&mut short);
    let index_result = {
        let mut view = PinDeviceViewMut::new(&mut indexed)?;
        view.lanes_mut(2).err()
    };
    let overflow_result = PinControlViewMut::new(&mut field)?.set_mode(8);

    // Then: each typed error is literal and no failed operation mutates its bytes.
    assert!(matches!(
        short_result,
        Err(SchemaCodecError::BufferTooShort {
            required: 48,
            actual: 47,
        })
    ));
    assert_eq!(
        index_result,
        Some(SchemaCodecError::BufferTooShort {
            required: 2,
            actual: 2,
        })
    );
    assert_eq!(
        overflow_result,
        Err(RegisterValueError::FieldValueOverflow {
            location: PinControl::MODE_IDENTITY.at_bit(1),
            bit_width: 3,
            value: 8,
        })
    );
    assert_eq!(short, short_before);
    assert_eq!(indexed, indexed_before);
    assert_eq!(field, field_before);
    println!(
        "adverse short=required48/actual47 index=count2/actual2 overflow=mode8/width3 unchanged=true"
    );
    Ok(())
}
