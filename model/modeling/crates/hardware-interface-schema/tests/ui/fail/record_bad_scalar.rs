use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { size: 4, alignment: 1, fields: {
    value: i32 { id: 1, offset: 0, endian: Little },
} } }
fn main() {}
