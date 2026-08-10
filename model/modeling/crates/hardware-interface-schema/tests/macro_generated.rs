//! Generated record, register, and map behavior tests.

use std::collections::{BTreeSet, HashSet};

use hardware_interface_schema::{
    SchemaCodecError, record_schema, register_map, register_map::RegisterLocation, register_schema,
};

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
