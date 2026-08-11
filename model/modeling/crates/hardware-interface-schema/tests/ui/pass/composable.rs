use hardware_interface_schema::{
    field_schema, record_schema, register_field_schema, register_map, register_schema,
};

field_schema! { pub field LeU32 { scalar: u32, endian: Little } }
record_schema! { pub record Header {
    size: 8, alignment: 4,
    fields: {
        sequence: LeU32 { id: 1, offset: 0 },
        flags: u16 { id: 2, offset: 4, endian: Little },
    },
    records: {},
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
    reset: Constant(0), writable_mask: 0x1,
    fields: { enable: Enable { id: 1, lsb: 0 }, },
} }
register_schema! { pub register Status {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff,
    fields: {},
} }
register_map! { pub map DeviceMap {
    size: 64,
    fixed: { control: Control { id: 1, name: "control", offset: 0 }, },
    banks: { channel: { count: 2, base: 16, stride: 16, members: {
        control: Control { id: 2, name: "control", offset: 0 },
    } }, },
} }

fn main() {
    let mut records = [0_u8; 16];
    let mut pair = PairViewMut::new(&mut records).unwrap();
    pair.left_mut().unwrap().set_sequence(0x1122_3344).unwrap();
    pair.right_mut().unwrap().set_flags(0x5566).unwrap();
    assert_eq!(PairView::new(&records).unwrap().left().unwrap().sequence(), Ok(0x1122_3344));

    let mut registers = [0_u8; 64];
    DeviceMapViewMut::new(&mut registers)
        .unwrap()
        .control_mut()
        .unwrap()
        .set_enable(1)
        .unwrap();
    assert_eq!(DeviceMapView::new(&registers).unwrap().control().unwrap().enable(), Ok(1));
    assert_eq!(DeviceMapView::new(&registers).unwrap().channel_control(0).unwrap().enable(), Ok(0));
}
