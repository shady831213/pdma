//! Exact scalar access into generated owned register compositions.

use hardware_interface_schema::{RegisterLayout, register_map, register_schema};

register_schema! { register Byte {
    width: 1, endian: Little, access: ReadWrite,
    reset: Constant(0x12), writable_mask: 0xff, fields: {},
} }
register_schema! { register Half {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0x3456), writable_mask: 0xffff, fields: {},
} }
register_schema! { register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0x789a_bcde), writable_mask: 0xffff_ffff, fields: {},
} }
register_schema! { register Wide {
    width: 8, endian: Big, access: ReadOnly,
    reset: Constant(0x0123_4567_89ab_cdef), writable_mask: 0, fields: {},
} }

register_map! { map Lane {
    alignment: 8, extent: 32,
    placements: [byte: Byte, half: Half, word: Word, wide: Wide],
} }
register_map! { map Group {
    alignment: 8, extent: 80,
    placements: [lanes: Lane[2] { offset: 8 }],
} }
register_map! { map Device {
    alignment: 8, extent: 184,
    placements: [
        head: Byte,
        groups: Group[2] { offset: 16 },
        tail: Word { offset: 176 },
    ],
} }

#[test]
fn reads_exact_leaves_when_layouts_repeat_at_nested_offsets() {
    // Given
    let value = DeviceValues::default();
    let expected = [
        (0, 0x12),
        (24, 0x12),
        (26, 0x3456),
        (28, 0x789a_bcde),
        (32, 0x0123_4567_89ab_cdef),
        (56, 0x12),
        (104, 0x12),
        (136, 0x12),
        (138, 0x3456),
        (140, 0x789a_bcde),
        (144, 0x0123_4567_89ab_cdef),
        (176, 0x789a_bcde),
    ];

    // When
    let observed = expected.map(|(offset, _)| Device::read_register(&value, offset));

    // Then
    assert_eq!(observed, expected.map(|(_, raw)| Some(raw)));
}

#[test]
fn writes_only_selected_leaf_when_nested_instances_share_a_layout() {
    // Given
    let mut value = DeviceValues::default();
    let mut expected = value;
    expected.groups[1].lanes[1].half = HalfValues::new(0xfedc);

    // When
    let result = Device::write_register(&mut value, 138, 0xfedc);

    // Then
    assert_eq!(result, Some(()));
    assert_eq!(value, expected);
}

#[test]
fn writes_full_scalar_when_bus_permissions_are_read_only() {
    // Given
    let mut value = DeviceValues::default();
    let mut expected = value;
    expected.groups[0].lanes[0].wide = WideValues::new(u64::MAX);

    // When
    let result = Device::write_register(&mut value, 32, u64::MAX);

    // Then
    assert_eq!(result, Some(()));
    assert_eq!(value, expected);
}

#[test]
fn preserves_all_values_when_raw_scalar_exceeds_leaf_width() {
    for (offset, raw) in [(136, 0x100), (138, 0x1_0000), (140, 0x1_0000_0000)] {
        // Given
        let mut value = DeviceValues::default();
        let before = value;

        // When
        let result = Device::write_register(&mut value, offset, raw);

        // Then
        assert_eq!(result, None, "offset {offset}");
        assert_eq!(value, before);
    }
}

#[test]
fn rejects_reads_when_offsets_are_interiors_holes_or_outside_map() {
    // Given
    let value = DeviceValues::default();
    let offsets = [
        1,
        16,
        25,
        27,
        29,
        33,
        40,
        48,
        80,
        128,
        152,
        160,
        172,
        184,
        usize::MAX,
    ];

    // When
    let observed = offsets.map(|offset| Device::read_register(&value, offset));

    // Then
    assert_eq!(observed, [None; 15]);
}

#[test]
fn preserves_all_values_when_write_offset_has_no_exact_leaf() {
    for offset in [
        1,
        16,
        25,
        27,
        29,
        33,
        40,
        48,
        80,
        128,
        152,
        160,
        172,
        184,
        usize::MAX,
    ] {
        // Given
        let mut value = DeviceValues::default();
        let before = value;

        // When
        let result = Device::write_register(&mut value, offset, 1);

        // Then
        assert_eq!(result, None, "offset {offset}");
        assert_eq!(value, before);
    }
}
