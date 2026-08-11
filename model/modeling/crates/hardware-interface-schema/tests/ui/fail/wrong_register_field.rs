use hardware_interface_schema::{field_schema, register_schema};

field_schema! { pub field LeU32 { scalar: u32, endian: Little } }
register_schema! { pub register Bad {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: { enable: LeU32 { id: 1, lsb: 0 }, },
} }

fn main() {}
