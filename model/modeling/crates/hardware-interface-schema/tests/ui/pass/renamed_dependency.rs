use hardware_interface_schema as his;
use his::{field_schema, record_schema, register_field_schema, register_map, register_schema};

field_schema! { pub field AliasWord { scalar: u32, endian: Little } }
record_schema! { pub record AliasRecord { size: 4, alignment: 4, fields: {
    value: AliasWord { id: 1, offset: 0 },
}, records: {} } }
register_field_schema! { pub register_field AliasEnable { bit_width: 1 } }
register_schema! { pub register AliasControl {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: { enable: AliasEnable { id: 1, lsb: 0 }, },
} }
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
}
