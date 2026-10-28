use super::*;
use hw_model_schema::{RegisterBankError, RegisterWindow};

register_map! { pub map Mab {
    alignment: 4, extent: 16,
    placements: [pub control: Word, pub status: Word { offset: 8 }],
} }
register_map! { pub map Grouped(count: usize) {
    alignment: 4, placements: [pub header: Word, pub channels: Mab[count] { offset: 16 }],
} }
register_map! { pub map Intruding(count: usize) {
    alignment: 4, placements: [pub channels: Mab[count], pub intruder: Word { offset: 4 }],
} }

#[test]
fn runtime_map_rejects_a_sibling_inside_a_block_hole() {
    assert!(matches!(Intruding::new(2), Err(RegisterBankError::Overlap)));
}

#[test]
fn grouped_bank_preserves_field_addresses_and_hardware_codec_behavior() {
    let layout = Grouped::new(3).unwrap();
    let mut hardware = RegisterWindow::<Mab>::new(0);
    assert_eq!(layout.channels().stride(), 16);
    assert_eq!(layout.channels().offset(2), Some(48));
    layout
        .channels()
        .route(56, |index, offset| {
            assert_eq!(index, 2);
            hardware.write(offset, &73u32.to_le_bytes())
        })
        .unwrap();
    let bytes = hardware.values.to_bytes().unwrap();
    assert_eq!(bytes.get(8..12), Some(73u32.to_le_bytes().as_slice()));
    assert_eq!(MabValues::from_bytes(&bytes).unwrap(), hardware.values);
}
