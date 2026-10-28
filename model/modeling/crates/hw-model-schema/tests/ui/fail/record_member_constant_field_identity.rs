use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { field: u8, FIELD_IDENTITY: u8 },
} }
fn main() {}
