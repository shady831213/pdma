use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { encode_into: u8 },
} }
fn main() {}
