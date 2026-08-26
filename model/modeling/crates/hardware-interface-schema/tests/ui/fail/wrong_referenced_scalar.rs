use hardware_interface_schema::{record_schema, register_schema};

register_schema! { pub register Control {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0,
    fields: {},
} }
record_schema! { pub record Bad {
    alignment: 4, endian: Little,
    fields: { value: Control, },
} }

fn main() {}
