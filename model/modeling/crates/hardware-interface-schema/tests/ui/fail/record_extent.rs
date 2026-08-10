use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { size: 4, alignment: 1, fields: {
    value: u32 { id: 1, offset: 2, endian: Little },
} } }
fn main() {}
