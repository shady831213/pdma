use super::*;
use hw_model_schema::{RegisterAccessError, RegisterBankError, RegisterWindow, register_dispatch};

register_map! { pub map Holed {
    alignment: 4, extent: 16, placements: [pub left: Word, pub right: Word { offset: 8 }],
} }
register_map! { pub map Interleaved(count: usize) {
    alignment: 4, placements: [
        pub wide: Holed[count] { offset: 16 },
        pub holes: Channel[count] { offset: 20 },
    ],
} }
register_map! { pub map Overlap(count: usize) {
    alignment: 4, placements: [
        pub wide: Holed[count] { offset: 16 },
        pub overlapping: Channel[count] { offset: 24 },
    ],
} }
register_map! { pub map Misaligned(count: usize) {
    alignment: 4, placements: [pub children: Channel[count] { offset: 1 }],
} }
register_map! { pub map FixedChild {
    alignment: 4, placements: [pub child: Group(3)[1]],
} }
register_map! { pub map CursorNamed(cursor: usize) {
    alignment: 4, placements: [pub channels: Channel[cursor]],
} }
register_map! { pub map AlignedBlock {
    alignment: 8, extent: 12, placements: [pub left: Word, pub right: Word { offset: 8 }],
} }
register_map! { pub map AlignedDevice(channels: usize) {
    alignment: 8, placements: [pub channels: AlignedBlock[channels] { offset: 16 }],
} }
register_map! { pub map GapIntrusion(channels: usize) {
    alignment: 8, placements: [pub channels: AlignedBlock[channels], pub intruder: Word { offset: 12 }],
} }
register_map! { pub map LocalNames(child: usize, alignment: usize, offset: usize, stride: usize) {
    alignment: 4, placements: [
        pub child: Channel[child],
        pub alignment: Channel[alignment],
        pub offset: Channel[offset],
        pub stride: Group(child)[stride],
    ],
} }

#[test]
fn parameters_keep_their_values_when_named_like_generator_locals() {
    let layout = CursorNamed::new(3).unwrap();
    assert_eq!(layout.channels().count(), 3);
    assert_eq!(layout.extent(), 48);
    let locals = LocalNames::new(2, 3, 5, 7).unwrap();
    assert_eq!(locals.child().count(), 2);
    assert_eq!(locals.alignment().count(), 3);
    assert_eq!(locals.offset().count(), 5);
    assert_eq!(locals.stride().count(), 7);
    assert_eq!(locals.stride().child().lanes().count(), 2);
}
register_map! { pub map Triplet {
    alignment: 4, extent: 16, placements: [
        pub left: Word, pub middle: Word { offset: 4 }, pub right: Word { offset: 8 },
    ],
} }
register_map! { pub map ReversedTriplet {
    alignment: 4, extent: 16, placements: [
        pub middle: Word { offset: 4 }, pub left: Word { offset: 0 }, pub right: Word { offset: 8 },
    ],
} }
register_map! { pub map FixedGrouped { alignment: 4, placements: [pub channels: Triplet[2]] } }
register_map! { pub map ReversedGrouped { alignment: 4, placements: [pub channels: ReversedTriplet[2]] } }

#[test]
fn encoding_preserves_grouped_members_declared_out_of_offset_order() {
    let mut window = RegisterWindow::<ReversedGrouped>::new(0);
    window.write(20, &67u32.to_le_bytes()).unwrap();
    window.write(24, &83u32.to_le_bytes()).unwrap();
    let bytes = window.values.to_bytes().unwrap();
    assert_eq!(bytes.get(20..24), Some(67u32.to_le_bytes().as_slice()));
    assert_eq!(bytes.get(24..28), Some(83u32.to_le_bytes().as_slice()));
    assert_eq!(
        ReversedGroupedValues::from_bytes(&bytes).unwrap(),
        window.values
    );
}

#[test]
fn nested_mutable_views_preserve_grouped_sibling_bytes() {
    let mut bytes = [0; ReversedGrouped::EXTENT];
    let mut view = ReversedGroupedViewMut::new(&mut bytes).unwrap();
    view.channels_mut(1)
        .unwrap()
        .middle_mut()
        .unwrap()
        .set_value(67)
        .unwrap();
    view.channels_mut(1)
        .unwrap()
        .right_mut()
        .unwrap()
        .set_value(83)
        .unwrap();
    assert_eq!(bytes.get(20..24), Some(67u32.to_le_bytes().as_slice()));
    assert_eq!(bytes.get(24..28), Some(83u32.to_le_bytes().as_slice()));
}

#[test]
fn fixed_maps_route_fields_within_complete_grouped_blocks() {
    let mut window = RegisterWindow::<FixedGrouped>::new(0);
    window.write(20, &67u32.to_le_bytes()).unwrap();
    window.write(24, &83u32.to_le_bytes()).unwrap();
    let mut bytes = [0; 4];
    window.read(20, &mut bytes).unwrap();
    assert_eq!(u32::from_le_bytes(bytes), 67);
}

#[test]
fn rejects_invalid_geometry_before_metadata_expansion() {
    assert!(matches!(Device::new(0), Err(RegisterBankError::ZeroCount)));
    assert!(matches!(
        Device::new(usize::MAX),
        Err(RegisterBankError::Overflow)
    ));
    assert!(matches!(Overlap::new(3), Err(RegisterBankError::Overlap)));
    assert!(matches!(
        GapIntrusion::new(2),
        Err(RegisterBankError::Overlap)
    ));
    assert!(matches!(
        Misaligned::new(1),
        Err(RegisterBankError::Alignment)
    ));
    assert_eq!(FixedChild::new().unwrap().extent(), 48);
}

#[test]
fn rejects_interleaved_composites_and_routes_complete_grouped_windows() {
    assert!(matches!(
        Interleaved::new(3),
        Err(RegisterBankError::Overlap)
    ));
    let layout = super::contiguous::Grouped::new(3).unwrap();
    let mut first = RegisterWindow::<super::contiguous::Mab>::new(0);
    let mut second = RegisterWindow::<super::contiguous::Mab>::new(0);
    let result = register_dispatch!(
        layout.channels().route(40, |index, relative| {
            assert_eq!(index, 1);
            second.write(relative, &91u32.to_le_bytes())
        }),
        Err(layout.finish_access(40)),
    );
    assert_eq!(result, Ok(()));
    let mut bytes = [0; 4];
    second.read(8, &mut bytes).unwrap();
    assert_eq!(u32::from_le_bytes(bytes), 91);
    first.read(8, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
}

#[test]
fn validation_and_hook_errors_preserve_output_and_hardware() {
    let layout = Device::new(3).unwrap();
    let mut window = RegisterWindow::<Channel>::new(0);
    let mut called = false;
    let mut output = [0xa5; 2];
    let result = layout.channels().route(16, |_, relative| {
        window.read_with(
            relative,
            &mut output,
            |_, _| {
                called = true;
                Ok::<(), u8>(())
            },
            |_, _| {},
        )
    });
    assert_eq!(
        result,
        Err(RegisterAccessError::WidthMismatch {
            offset: 0,
            expected: 4,
            actual: 2
        })
    );
    assert!(!called);
    assert_eq!(output, [0xa5; 2]);
    let result = layout.channels().route(32, |_, relative| {
        window.write_with(relative, &1u32.to_le_bytes(), |_, _, _| Err(7u8), |_, _| {})
    });
    assert_eq!(result, Err(RegisterAccessError::Other(7)));
    assert_eq!(window.values, ChannelValues::DEFAULT);
}

#[test]
fn misses_preserve_original_address_and_distinguish_final_padding() {
    let layout = AlignedDevice::new(3).unwrap();
    let bank = layout.channels().rebased(0x1000).unwrap();
    let mut window = RegisterWindow::<AlignedBlock>::new(0);
    let result = bank.route(0x1014, |_, offset| window.read(offset, &mut [0; 4]));
    assert_eq!(
        result,
        Err(RegisterAccessError::InvalidOffset { offset: 4 })
    );
    assert_eq!(
        bank.finish_access::<()>(0x1014),
        RegisterAccessError::InvalidOffset { offset: 4 }
    );
    assert_eq!(
        bank.finish_access::<()>(0x103c),
        RegisterAccessError::OutsideWindow { address: 0x103c }
    );
}

#[test]
fn slot_selection_includes_final_padding_before_hardware_permission_checks() {
    let layout = AlignedDevice::new(3).unwrap();
    assert_eq!(layout.channels().locate_instance(20), Some((0, 4)));
    assert_eq!(layout.channels().locate_instance(63), Some((2, 15)));
    assert_eq!(layout.channels().locate_instance(64), None);
    assert_eq!(layout.channels().locate(63), None);
    assert_eq!(
        layout.channels().finish_access::<()>(63),
        RegisterAccessError::OutsideWindow { address: 63 }
    );
}
