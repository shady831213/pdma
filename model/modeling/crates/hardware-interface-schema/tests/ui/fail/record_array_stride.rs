use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { size: 8, alignment: 1, fields: {
    values: [u16; 2] { ids: [1, 2], offset: 0, stride: 1, endian: Little },
} } }
fn main() {}
