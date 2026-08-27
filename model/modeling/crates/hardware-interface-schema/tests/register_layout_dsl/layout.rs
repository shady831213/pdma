use super::*;

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
fn ordered_resolution_flattens_nested_leaf_registers_from_the_same_placements()
-> Result<(), Box<dyn std::error::Error>> {
    let resolved = DeviceRegisters::resolve()?;
    let paths = resolved
        .registers()
        .iter()
        .map(|register| (register.path(), register.offset()))
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            ("DeviceRegisters.header", 0),
            ("DeviceRegisters.lanes[0].control", 8),
            ("DeviceRegisters.lanes[0].counter", 16),
            ("DeviceRegisters.lanes[1].control", 24),
            ("DeviceRegisters.lanes[1].counter", 32),
            ("DeviceRegisters.tail", 56),
        ]
    );
    assert_eq!(
        resolved
            .locate(24)
            .map(hardware_interface_schema::ResolvedRegister::path),
        Some("DeviceRegisters.lanes[1].control")
    );
    assert_eq!(resolved.locate(25), None);
    Ok(())
}

#[test]
fn generated_map_layout_uses_the_declared_endian_and_exact_explicit_offset() {
    assert_eq!(ControlRegister::SPEC.endian(), Endian::Little);
    assert_eq!(DeviceRegisters::TAIL.offset(), 56);
}
