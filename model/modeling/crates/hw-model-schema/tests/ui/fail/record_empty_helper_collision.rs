use hw_model_schema::record_schema;
record_schema! { pub record Collision {
    endian: Little, fields: { __: enum(u8) { variants: { Value = 1 } } },
} }
fn main() {}
