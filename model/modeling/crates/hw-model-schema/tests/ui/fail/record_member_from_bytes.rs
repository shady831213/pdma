use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { from_bytes: u8 },
} }
fn main() {}
