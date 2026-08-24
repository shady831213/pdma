use hardware_interface_schema as his;
use his::{bits_schema, field_schema, record_schema, register_map, register_schema};

field_schema! { pub field AliasWord { scalar: u32, endian: Little } }
bits_schema! { pub bits AliasBits { enable: 0..1, } }
record_schema! { pub record AliasRecord { size: 4, alignment: 4, fields: {
    value: AliasWord { id: 1, offset: 0 },
}, records: {} } }
register_schema! { pub register AliasControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: AliasBits,
} }
record_schema! { pub record AliasBitsRecord { endian: Little, fields: {
    flags: bits(AliasBits),
} } }
register_map! { pub map AliasMap { size: 4, fixed: {
    control: AliasControl { id: 1, name: "control", offset: 0 },
}, banks: {} } }

record_schema! { pub record RenamedRecord { size: 1, alignment: 1, fields: {
    byte: u8 { id: 1, offset: 0, endian: Little },
} } }
register_schema! { pub registers RenamedRegisters {
    byte: { id: 1, name: "byte", offset: 0, width: 1, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
} }
register_map! { pub map RenamedMap { size: 1, fixed: {
    byte: { id: 1, name: "byte", offset: 0, width: 1, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
}, banks: {} } }

fn main() {
    assert_eq!(RenamedMap::SPEC.size(), 1);
    assert_eq!(AliasRecord::SPEC.size(), 4);
    assert_eq!(AliasMap::SPEC.size(), 4);
    assert_eq!(AliasBitsRecordValues { flags: AliasBitsValues { enable: true } }.to_bytes().unwrap(), [1]);
}
