use hw_model_schema::register_schema;
register_schema! { pub register Conflict {
    width: 4, endian: Little, access: ReadWrite,
    reset: Zero(0), writable_mask: 1,
    fields: { set_value: 0..=0 },
} }
fn main() {}
