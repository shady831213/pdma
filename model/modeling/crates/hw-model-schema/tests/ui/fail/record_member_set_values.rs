use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: { set_values: u8 },
} }
fn main() {}
