use hardware_interface_schema::{bits_schema, record_schema, register_map, register_schema};

bits_schema! { pub bits PassBits { enable: 0..1, } }

record_schema! { pub record PassRecord { size: 8, alignment: 4, fields: {
    value: u32 { id: 1, offset: 0, endian: Little },
    items: [u16; 2] { ids: [2, 3], offset: 4, stride: 2, endian: Big },
} } }
register_schema! { pub registers PassRegisters {
    value: { id: 1, name: "value", offset: 0, width: 4, access: ReadWrite,
        reset: Constant(0), writable_mask: 0xffff_ffff },
} }
register_schema! { pub register PassControl {
    width: 1, endian: Little, access: ReadWrite,
    reset: Zero(0), writable_mask: 1, fields: PassBits,
} }
record_schema! { pub record PassInferredRecord { endian: Little, fields: {
    flags: bits(PassBits),
} } }
register_map! { pub map PassMap { size: 32, fixed: {
    value: { id: 1, name: "value", offset: 0, width: 4, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
}, banks: { lane: { count: 2, base: 8, stride: 8, members: {
    data: { id: 2, name: "data", offset: 0, width: 4, access: ReadWrite,
        reset: PostReset(0), writable_mask: 0xffff_ffff },
} }, } } }

fn main() {
    let mut bytes = [0; 8];
    let mut view = PassRecordViewMut::new(&mut bytes).unwrap();
    view.set_items(1, 0x1234).unwrap();
    let flags = PassBitsValues { enable: true };
    let mut control = [0_u8; 1];
    PassControlViewMut::new(&mut control).unwrap().set_fields(flags).unwrap();
    assert_eq!(PassInferredRecordValues { flags }.to_bytes().unwrap(), [1]);
    assert!(matches!(PassMap::locate(16), Some(hardware_interface_schema::register_map::RegisterLocation::Repeated { instance: 1, .. })));
}
