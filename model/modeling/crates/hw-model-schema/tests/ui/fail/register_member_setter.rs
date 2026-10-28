use hw_model_schema::register_schema;
register_schema! { pub register Conflict {
    width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 3,
    fields: { foo: 0..=0, set_foo: 1..=1 },
} }
fn main() {}
