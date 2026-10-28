use hw_model_schema::register_schema;
register_schema! { pub register Bad {
    width: 1, endian: Little, access: ReadWrite,
    reset: Zero(0), writable_mask: 0x100, fields: {},
} }
fn main() {}
