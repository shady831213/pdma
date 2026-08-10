use hardware_interface_schema as his;
use his::{record_schema, register_map, register_schema};

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

fn main() { assert_eq!(RenamedMap::SPEC.size(), 1); }
