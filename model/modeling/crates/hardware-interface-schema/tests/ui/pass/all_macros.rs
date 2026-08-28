use hardware_interface_schema::{bits_schema, record_schema, register_map, register_schema};

bits_schema! { pub bits PassBits(u8) { enable: 0..1, } }

record_schema! { pub record PassItems { endian: Big, fields: {
    item0: u16,
    item1: u16,
} } }
record_schema! { pub record PassRecord { alignment: 4, endian: Little, fields: {
    value: u32,
    items: PassItems,
} } }
register_schema! { pub register PassValue {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_schema! { pub register PassControl {
    width: 1, endian: Little, access: ReadWrite,
    reset: Zero(0), writable_mask: 1, fields: PassBits,
} }
record_schema! { pub record PassInferredRecord { endian: Little, fields: {
    flags: bits(PassBits),
} } }
register_schema! { pub register PassStatus {
    width: 4, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {},
} }
register_schema! { pub register PassData {
    width: 4, endian: Little, access: ReadWrite,
    reset: PostReset(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_map! { pub map PassLane { alignment: 4, placements: [data: PassData], } }
register_map! { pub map PassMap {
    alignment: 4,
    extent: 32,
    placements: [
        value: PassStatus,
        lane: PassLane { offset: 8, count: 2, stride: 8 },
    ],
} }

fn main() {
    let mut bytes = [0; 8];
    let mut view = PassRecordViewMut::new(&mut bytes).unwrap();
    view.items_mut().unwrap().set_item1(0x1234).unwrap();
    let flags = PassBitsValues { enable: true };
    let mut control = [0_u8; 1];
    PassControlViewMut::new(&mut control).unwrap().set_fields(flags).unwrap();
    assert_eq!(PassInferredRecord { flags }.to_bytes().unwrap(), [1]);
    assert!(matches!(PassMap::locate(16), Some(hardware_interface_schema::register_map::RegisterLocation::Repeated { instance: 1, .. })));
}
