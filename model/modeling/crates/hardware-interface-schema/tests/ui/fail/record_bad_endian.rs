use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { alignment: 1, endian: Native, fields: {
    value: u32,
} } }
fn main() {}
