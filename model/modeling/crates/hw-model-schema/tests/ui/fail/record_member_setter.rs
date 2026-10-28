use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little,
    fields: { foo: u8, set_foo: u8 },
} }
fn main() {}
