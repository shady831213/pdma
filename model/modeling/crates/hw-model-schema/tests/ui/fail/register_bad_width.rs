use hw_model_schema::register_schema;
register_schema! { pub register Bad {
    width: 3, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {},
} }
fn main() {}
