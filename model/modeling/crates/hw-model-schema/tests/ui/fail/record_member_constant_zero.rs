use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { field: u8, ZERO: u8 },
} }
fn main() {}
