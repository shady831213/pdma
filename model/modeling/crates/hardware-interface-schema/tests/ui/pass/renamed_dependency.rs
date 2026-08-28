use hardware_interface_schema as his;
use his::{bits_schema, record_schema, register_map, register_schema};

bits_schema! { pub bits AliasBits(u8) { enable: 0..1, } }
record_schema! { pub record AliasRecord { alignment: 4, endian: Little, fields: {
    value: u32,
} } }
register_schema! { pub register AliasControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: AliasBits,
} }
record_schema! { pub record AliasBitsRecord { endian: Little, fields: {
    flags: bits(AliasBits),
} } }
register_map! { pub map AliasMap {
    alignment: 4, extent: 4, placements: [control: AliasControl],
} }

record_schema! { pub record RenamedRecord { alignment: 1, endian: Little, fields: {
    byte: u8,
} } }
register_schema! { pub register RenamedByte {
    width: 1, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {},
} }
register_map! { pub map RenamedMap {
    alignment: 1, extent: 1, placements: [byte: RenamedByte],
} }

fn main() {
    assert_eq!(RenamedMap::EXTENT, 1);
    assert_eq!(AliasRecord::SPEC.size(), 4);
    assert_eq!(AliasMap::EXTENT, 4);
    assert_eq!(AliasBitsRecord { flags: AliasBitsValues { enable: true } }.to_bytes().unwrap(), [1]);
}
