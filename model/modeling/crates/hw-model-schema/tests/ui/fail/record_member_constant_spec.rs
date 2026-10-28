use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { field: u8, SPEC: u8 },
} }
fn main() {}
