use hw_model_schema::record_schema;
record_schema! { pub record Bad { alignment: 1, endian: Little, fields: {
    value: i32,
} } }
fn main() {}
