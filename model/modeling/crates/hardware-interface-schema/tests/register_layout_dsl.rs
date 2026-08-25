//! Ordered register-layout macro behavior.

use hardware_interface_schema::{
    Decode, Encode, Endian, RegisterLayout, register_map, register_schema,
};

register_schema! {
    pub register ControlRegister {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0x1020_3040),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

register_schema! {
    pub register CounterRegister {
        width: 8,
        alignment: 8,
        endian: Little,
        access: ReadOnly,
        reset: PostReset(0x0102_0304_0506_0708),
        writable_mask: 0,
        fields: {},
    }
}

register_map! {
    pub map LaneRegisters {
        alignment: 8,
        placements: [
            control: ControlRegister,
            counter: CounterRegister,
        ],
    }
}

register_map! {
    pub map DeviceRegisters {
        alignment: 16,
        extent: 64,
        placements: [
            header: ControlRegister,
            lanes: LaneRegisters { count: 2 },
            tail: ControlRegister { offset: 56 },
        ],
    }
}

#[test]
fn ordered_placements_infer_aligned_offsets_extent_and_stride() {
    assert_eq!(ControlRegister::SPEC.alignment(), 4);
    assert_eq!(<ControlRegister as RegisterLayout>::ALIGNMENT, 4);
    assert_eq!(LaneRegisters::ALIGNMENT, 8);
    assert_eq!(LaneRegisters::EXTENT, 16);
    assert_eq!(LaneRegisters::CONTROL.offset(), 0);
    assert_eq!(LaneRegisters::COUNTER.offset(), 8);

    assert_eq!(DeviceRegisters::ALIGNMENT, 16);
    assert_eq!(DeviceRegisters::EXTENT, 64);
    assert_eq!(DeviceRegisters::HEADER.offset(), 0);
    assert_eq!(DeviceRegisters::LANES.offset(), 8);
    assert_eq!(DeviceRegisters::LANES.count(), 2);
    assert_eq!(DeviceRegisters::LANES.stride(), 16);
    assert_eq!(DeviceRegisters::TAIL.offset(), 56);
}

#[test]
fn generated_owned_values_round_trip_nested_map_bytes() {
    let values = DeviceRegistersValues {
        header: ControlRegisterValues::new(0x1122_3344),
        lanes: [
            LaneRegistersValues {
                control: ControlRegisterValues::new(7),
                counter: CounterRegisterValues::new(9),
            },
            LaneRegistersValues {
                control: ControlRegisterValues::new(11),
                counter: CounterRegisterValues::new(13),
            },
        ],
        tail: ControlRegisterValues::new(0xaabb_ccdd),
    };
    let encoded = values.to_bytes();
    assert!(encoded.is_ok());
    let Ok(bytes) = encoded else {
        return;
    };
    assert_eq!(&bytes[0..4], &0x1122_3344_u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &7_u32.to_le_bytes());
    assert_eq!(&bytes[16..24], &9_u64.to_le_bytes());
    assert_eq!(&bytes[24..28], &11_u32.to_le_bytes());
    assert_eq!(&bytes[32..40], &13_u64.to_le_bytes());
    assert_eq!(&bytes[56..60], &0xaabb_ccdd_u32.to_le_bytes());
    assert_eq!(DeviceRegistersValues::from_bytes(&bytes), Ok(values));
    let view = DeviceRegistersView::new(&bytes);
    assert!(view.is_ok());
    let Ok(view) = view else {
        return;
    };
    let second_lane = view.lanes(1);
    assert!(second_lane.is_ok());
    let Ok(second_lane) = second_lane else {
        return;
    };
    let counter = second_lane.counter();
    assert!(counter.is_ok());
    let Ok(counter) = counter else {
        return;
    };
    assert_eq!(counter.value(), Ok(13));
}

#[test]
fn generated_map_default_recursively_composes_register_resets()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: nested and repeated registers with nonzero declared reset values.
    let values = DeviceRegistersValues::default();

    // When: the reset-derived semantic map is encoded and decoded.
    let raw = values.encode()?;
    let decoded = raw.decode()?;
    let converted = DeviceRegistersValues::try_from(raw)?;

    // Then: every child reset occupies its inferred placement and padding stays zero.
    assert_eq!(values.header.raw(), 0x1020_3040);
    assert_eq!(values.lanes[0].control.raw(), 0x1020_3040);
    assert_eq!(values.lanes[0].counter.raw(), 0x0102_0304_0506_0708);
    assert_eq!(values.lanes[1], values.lanes[0]);
    assert_eq!(values.tail.raw(), 0x1020_3040);
    assert_eq!(&raw.as_bytes()[0..4], &0x1020_3040_u32.to_le_bytes());
    assert_eq!(
        &raw.as_bytes()[16..24],
        &0x0102_0304_0506_0708_u64.to_le_bytes()
    );
    assert!(raw.as_bytes()[40..56].iter().all(|byte| *byte == 0));
    assert_eq!(decoded, values);
    assert_eq!(converted, values);
    Ok(())
}

#[test]
fn generated_map_layout_uses_the_declared_endian_and_exact_explicit_offset() {
    assert_eq!(ControlRegister::SPEC.endian(), Endian::Little);
    assert_eq!(DeviceRegisters::TAIL.offset(), 56);
}

#[test]
fn generated_owned_values_reject_short_output_before_mutation() {
    let values = DeviceRegistersValues::ZERO;
    let mut bytes = [0xa5; 63];
    let before = bytes;

    assert!(values.encode_into(&mut bytes).is_err());
    assert_eq!(bytes, before);
}
