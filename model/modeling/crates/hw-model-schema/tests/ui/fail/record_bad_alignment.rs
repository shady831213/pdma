use hw_model_schema::record_schema;
record_schema! { pub record Bad { alignment: 3, endian: Little, fields: {
    value: u32,
} } }
fn main() {}
