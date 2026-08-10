use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { size: 8, alignment: 1, fields: {
    first: u32 { id: 1, offset: 0, endian: Little },
    second: u32 { id: 2, offset: 2, endian: Little },
} } }
fn main() {}
