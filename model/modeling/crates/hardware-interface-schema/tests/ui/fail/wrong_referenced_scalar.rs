use hardware_interface_schema::{record_schema, register_schema};

register_schema! { pub register Control {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0,
    fields: {},
} }
record_schema! { pub record Bad {
    size: 4, alignment: 4,
    fields: { value: Control { id: 1, offset: 0 }, },
    records: {},
} }

fn main() {}
