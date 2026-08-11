use hardware_interface_schema::{register_field_schema, register_schema};

register_field_schema! { pub register_field Wide { bit_width: 8 } }
register_schema! { pub register Bad {
    width: 1, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xff,
    fields: { wide: Wide { id: 1, lsb: 1 }, },
} }

fn main() {}
