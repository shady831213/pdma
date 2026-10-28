use hw_model_schema::{record_schema, register_map, register_schema};

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
    reset: Constant(0), writable_mask: 0x1,
    fields: { enable: 0..1, },
} }
register_schema! { pub register Status {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map ChannelMap {
    alignment: 4, extent: 16,
    placements: [control: Control],
} }
register_map! { pub map DeviceMap {
    alignment: 4,
    extent: 64,
    placements: [
        control: Control,
        channel: ChannelMap[2] { offset: 16 },
    ],
} }

fn main() {
    let mut records = [0_u8; 16];
    let mut pair = PairViewMut::new(&mut records).unwrap();
    pair.left_mut().unwrap().set_sequence(0x1122_3344).unwrap();
    pair.right_mut().unwrap().set_flags(0x5566).unwrap();
    assert_eq!(
        PairView::new(&records).unwrap().left().unwrap().sequence(),
        Ok(0x1122_3344)
    );

    let mut registers = [0_u8; 64];
    DeviceMapViewMut::new(&mut registers)
        .unwrap()
        .control_mut()
        .unwrap()
        .set_enable(true)
        .unwrap();
    assert_eq!(
        DeviceMapView::new(&registers)
            .unwrap()
            .control()
            .unwrap()
            .enable(),
        Ok(true)
    );
    assert_eq!(
        DeviceMapView::new(&registers)
            .unwrap()
            .channel(0)
            .unwrap()
            .control()
            .unwrap()
            .enable(),
        Ok(false)
    );
}
