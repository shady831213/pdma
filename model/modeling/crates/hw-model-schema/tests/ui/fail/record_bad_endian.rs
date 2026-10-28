use hw_model_schema::record_schema;
record_schema! { pub record Bad { alignment: 1, endian: Native, fields: {
    value: u32,
} } }
fn main() {}
