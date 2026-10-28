use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { to_bytes: u8 },
} }
fn main() {}
