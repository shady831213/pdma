//! Generated record, register, and map behavior tests.

use std::collections::{BTreeSet, HashSet};

use hardware_interface_schema::{
    RegisterValueError, SchemaCodecError, field_schema, record_schema, register_field_schema,
    register_map, register_map::RegisterLocation, register_schema,
};

field_schema! { pub field LeU32 { scalar: u32, endian: Little } }
record_schema! { pub record Header {
    size: 8, alignment: 4,
    fields: {
        sequence: LeU32 { id: 1, offset: 0 },
        flags: u16 { id: 2, offset: 4, endian: Little },
    }, records: {},
} }
record_schema! { pub record Pair {
    size: 16, alignment: 8, fields: {},
    records: {
        left: Header { id: 1, offset: 0 },
        right: Header { id: 2, offset: 8 },
    },
} }
register_field_schema! { pub register_field Enable { bit_width: 1 } }
register_schema! { pub register Control {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: { enable: Enable { id: 1, lsb: 0 }, },
} }
register_schema! { pub register BeStatus {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map ComposableMap {
    size: 64,
    fixed: { control: Control { id: 1, name: "control", offset: 0 }, },
    banks: { channel: { count: 2, base: 16, stride: 16, members: {
        status: BeStatus { id: 2, name: "status", offset: 0 },
    } }, },
} }

record_schema! {
    pub record Telemetry {
        size: 16,
        alignment: 8,
        fields: {
            sequence: u32 { id: 1, offset: 0, endian: Little },
            code: u16 { id: 2, offset: 4, endian: Big },
            samples: [u16; 2] { ids: [3, 4], offset: 8, stride: 2, endian: Little },
        }
    }
}

register_schema! {
    pub registers Controls {
        status: { id: 1, name: "status", offset: 0, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
        command: { id: 2, name: "command", offset: 4, width: 2, access: WriteOnly,
            reset: PostReset(0x12), writable_mask: 0xff },
    }
}

register_map! {
    pub map SensorMap {
        size: 64,
        fixed: {
            status: { id: 1, name: "status", offset: 0, width: 4, access: ReadOnly,
                reset: Zero(0), writable_mask: 0 },
        },
        banks: {
            channel: { count: 2, base: 16, stride: 16, members: {
                data: { id: 2, name: "data", offset: 0, width: 4, access: ReadWrite,
                    reset: Constant(0), writable_mask: 0xffff_ffff },
                flags: { id: 3, name: "flags", offset: 8, width: 1, access: ReadOnly,
                    reset: Zero(0), writable_mask: 0 },
            } },
        }
    }
}

#[test]
fn record_accessors_when_mixed_endian_and_array_are_used() -> Result<(), SchemaCodecError> {
    let mut bytes = [0_u8; 16];
    let mut view = TelemetryViewMut::new(&mut bytes)?;
    view.set_sequence(0x1122_3344)?;
    view.set_code(0x5566)?;
    view.set_samples(0, 0x7788)?;
    view.set_samples(1, 0x99aa)?;
    assert_eq!(view.sequence()?, 0x1122_3344);
    assert_eq!(
        &bytes[..12],
        &[
            0x44, 0x33, 0x22, 0x11, 0x55, 0x66, 0, 0, 0x88, 0x77, 0xaa, 0x99
        ]
    );
    let view = TelemetryView::new(&bytes)?;
    assert_eq!(view.sequence()?, 0x1122_3344);
    assert_eq!(view.code()?, 0x5566);
    assert_eq!(view.samples(1)?, 0x99aa);
    assert_eq!(Telemetry::SEQUENCE_OFFSET, 0);
    assert_eq!(Telemetry::SAMPLES_OFFSET, 8);
    assert_eq!(Telemetry::SPEC.fields().len(), 4);
    Ok(())
}

#[test]
fn generated_ids_when_used_in_hash_and_order_sets() {
    let hash = HashSet::from([TelemetryFieldId::Sequence, TelemetryFieldId::Samples0]);
    let order = BTreeSet::from([
        SensorMapRegisterId::ChannelData,
        SensorMapRegisterId::Status,
    ]);
    assert_eq!(hash.len(), 2);
    assert_eq!(order.len(), 2);
}

#[test]
fn register_metadata_when_schema_is_generated() {
    let command = Controls::COMMAND;
    assert_eq!(command.id(), ControlsRegisterId::Command);
    assert_eq!(command.numeric_id(), Some(2));
    assert_eq!(command.reset_value(), 0x12);
    assert_eq!(
        Controls::register(ControlsRegisterId::Status),
        Controls::STATUS
    );
}

#[test]
fn map_lookup_when_fixed_repeated_interior_and_unmapped_offsets_are_used() {
    assert_eq!(
        SensorMap::register(SensorMapRegisterId::ChannelFlags).name(),
        "flags"
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
    assert_eq!(Pair::LEFT_DEFINITION.name(), "Header");
    Ok(())
}

#[test]
fn register_map_views_when_le_and_be_definitions_are_placed_use_exact_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 64];
    let mut map = ComposableMapViewMut::new(&mut bytes)?;
    map.control_mut()?.set_enable(1)?;
    map.channel_status_mut(0)?.set_value(0x1234)?;
    map.channel_status_mut(1)?.set_value(0xabcd)?;
    assert_eq!(&bytes[..4], &[1, 0, 0, 0]);
    assert_eq!(&bytes[16..18], &[0x12, 0x34]);
    assert_eq!(&bytes[32..34], &[0xab, 0xcd]);
    let map = ComposableMapView::new(&bytes)?;
    assert_eq!(map.control()?.enable()?, 1);
    assert_eq!(map.channel_status(1)?.value()?, 0xabcd);
    Ok(())
}

#[test]
fn generated_checked_views_when_buffers_or_values_are_invalid_do_not_mutate()
-> Result<(), RegisterValueError> {
    let mut raw = 0_u64;
    Enable::insert(&mut raw, 1)?;
    assert_eq!(Enable::extract(raw), 1);
    assert!(matches!(
        Enable::insert(&mut raw, 2),
        Err(RegisterValueError::FieldValueOverflow { .. })
    ));
    assert_eq!(raw, 1);
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
    let mut control = ControlViewMut::new(&mut value)?;
    assert!(matches!(
        control.set_enable(2),
        Err(RegisterValueError::FieldValueOverflow { .. })
    ));
    assert_eq!(value, [0; 4]);
    Ok(())
}
