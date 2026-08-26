use hardware_interface_schema::register_schema;
register_schema! { pub register Bad {
    width: 1, endian: Little, access: ReadOnly,
    reset: Constant(0x100), writable_mask: 0, fields: {},
} }
fn main() {}
