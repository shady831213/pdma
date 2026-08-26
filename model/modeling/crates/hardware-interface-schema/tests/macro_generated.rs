//! Generated record, register, and map behavior tests.

use std::collections::{BTreeSet, HashSet};

use hardware_interface_schema::{
    RegisterValueError, SchemaCodecError, record_schema, register_map,
    register_map::RegisterLocation, register_schema,
};

record_schema! { pub record Header {
    alignment: 4, endian: Little,
    fields: {
        sequence: u32,
        flags: u16,
    },
} }
record_schema! { pub record Pair {
    alignment: 8, endian: Little,
    fields: {
        left: Header,
        right: Header,
    },
} }
register_schema! { pub register Control {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: { enable: 0..1, },
} }
register_schema! { pub register BeStatus {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map ComposableMap {
    alignment: 4,
    extent: 64,
    placements: [
        control: Control,
        channel_status: BeStatus { offset: 16, count: 2, stride: 16 },
    ],
} }

record_schema! { pub record TelemetrySamples {
    alignment: 4,
    endian: Little,
    fields: {
        sample0: u16,
        sample1: u16,
    },
} }

record_schema! {
    pub record Telemetry {
        alignment: 8,
        endian: Little,
        fields: {
            sequence: u32,
            code: u16,
            samples: TelemetrySamples,
        }
    }
}

register_schema! {
    pub register StatusRegister {
        width: 4, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {},
    }
}

register_schema! {
    pub register CommandRegister {
        width: 2, endian: Little, access: WriteOnly,
        reset: PostReset(0x12), writable_mask: 0xff, fields: {},
    }
}

register_schema! {
    pub register SensorData {
        width: 4, endian: Little, access: ReadWrite,
        reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
    }
}

register_schema! {
    pub register SensorFlags {
        width: 1, endian: Little, access: ReadOnly,
        reset: Zero(0), writable_mask: 0, fields: {},
    }
}

register_map! {
    pub map SensorMap {
        alignment: 4,
        extent: 64,
        placements: [
            status: StatusRegister,
            channel_data: SensorData { offset: 16, count: 2, stride: 16 },
            channel_flags: SensorFlags { offset: 24, count: 2, stride: 16 },
        ],
    }
}

#[test]
fn record_accessors_when_nested_sequence_is_used() -> Result<(), SchemaCodecError> {
    let mut bytes = [0_u8; 16];
    let mut view = TelemetryViewMut::new(&mut bytes)?;
    view.set_sequence(0x1122_3344)?;
    view.set_code(0x5566)?;
    view.samples_mut()?.set_sample0(0x7788)?;
    view.samples_mut()?.set_sample1(0x99aa)?;
    assert_eq!(view.sequence()?, 0x1122_3344);
    assert_eq!(
        &bytes[..12],
        &[
            0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0, 0, 0x88, 0x77, 0xaa, 0x99
        ]
    );
    let view = TelemetryView::new(&bytes)?;
    assert_eq!(view.sequence()?, 0x1122_3344);
    assert_eq!(view.code()?, 0x5566);
    assert_eq!(view.samples()?.sample1()?, 0x99aa);
    assert_eq!(Telemetry::SEQUENCE_OFFSET, 0);
    assert_eq!(Telemetry::SAMPLES_OFFSET, 8);
    assert_eq!(Telemetry::SPEC.fields().len(), 2);
    assert_eq!(Telemetry::SPEC.records().len(), 1);
    assert_eq!(
        Telemetry::SPEC
            .resolve()
            .map(|resolved| resolved.members().len()),
        Ok(5)
    );
    Ok(())
}

#[test]
fn generated_ids_when_used_in_hash_and_order_sets() {
    let hash = HashSet::from([
        SensorMapRegisterId::ChannelFlags,
        SensorMapRegisterId::Status,
    ]);
    let order = BTreeSet::from([
        SensorMapRegisterId::ChannelData,
        SensorMapRegisterId::Status,
    ]);
    assert_eq!(hash.len(), 2);
    assert_eq!(order.len(), 2);
}

#[test]
fn register_metadata_when_schema_is_generated() {
    assert_eq!(StatusRegister::SPEC.reset_value(), 0);
    assert_eq!(CommandRegister::SPEC.reset_value(), 0x12);
    assert_eq!(CommandRegister::SPEC.writable_mask(), 0xff);
}

#[test]
fn map_lookup_when_fixed_repeated_interior_and_unmapped_offsets_are_used() {
    assert_eq!(
        SensorMap::register(SensorMapRegisterId::ChannelFlags)
            .map(hardware_interface_schema::RegisterSpec::name),
        Some("CHANNEL_FLAGS")
    );
    assert!(matches!(
        SensorMap::locate(0),
        Some(RegisterLocation::Fixed { .. })
    ));
    assert!(matches!(
        SensorMap::locate(32),
        Some(RegisterLocation::Repeated { instance: 1, .. })
    ));
    assert_eq!(SensorMap::locate(33), None);
    assert_eq!(SensorMap::locate(63), None);
}

#[test]
fn views_reject_short_buffers_when_constructed() {
    assert_eq!(
        TelemetryView::new(&[0; 15]),
        Err(SchemaCodecError::BufferTooShort {
            required: 16,
            actual: 15
        })
    );
}

#[test]
fn nested_record_views_when_definition_is_placed_twice_use_exact_bytes()
-> Result<(), SchemaCodecError> {
    let mut bytes = [0_u8; 16];
    let mut pair = PairViewMut::new(&mut bytes)?;
    pair.left_mut()?.set_sequence(0x1122_3344)?;
    pair.right_mut()?.set_flags(0x5566)?;
    assert_eq!(
        bytes,
        [
            0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0x66, 0x55, 0, 0
        ]
    );
    let pair = PairView::new(&bytes)?;
    assert_eq!(pair.left()?.sequence()?, 0x1122_3344);
    assert_eq!(pair.right()?.flags()?, 0x5566);
    assert_eq!(Pair::LEFT_IDENTITY.member(), "left");
    Ok(())
}

#[test]
fn register_map_views_when_le_and_be_definitions_are_placed_use_exact_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 64];
    let mut map = ComposableMapViewMut::new(&mut bytes)?;
    map.control_mut()?.set_enable(true)?;
    map.channel_status_mut(0)?.set_value(0x1234)?;
    map.channel_status_mut(1)?.set_value(0xabcd)?;
    assert_eq!(&bytes[..4], &[1, 0, 0, 0]);
    assert_eq!(&bytes[16..18], &[0x12, 0x34]);
    assert_eq!(&bytes[32..34], &[0xab, 0xcd]);
    let map = ComposableMapView::new(&bytes)?;
    assert!(map.control()?.enable()?);
    assert_eq!(map.channel_status(1)?.value()?, 0xabcd);
    Ok(())
}

#[test]
fn generated_checked_views_when_buffers_or_values_are_invalid_do_not_mutate()
-> Result<(), RegisterValueError> {
    let mut short = [0x5a_u8; 3];
    assert!(matches!(
        ControlViewMut::new(&mut short),
        Err(RegisterValueError::BufferTooShort {
            required: 4,
            actual: 3
        })
    ));
    assert_eq!(short, [0x5a; 3]);
    let mut value = [0_u8; 4];
    assert!(matches!(
        Control::SPEC
            .view_mut(&mut value)?
            .set_field_by_name("enable", 2),
        Err(RegisterValueError::FieldValueOverflow { .. })
    ));
    assert_eq!(value, [0; 4]);
    Ok(())
}
