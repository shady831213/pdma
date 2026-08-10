use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { size: 4, alignment: 1, fields: {
    same: u16 { id: 1, offset: 0, endian: Little },
    same: u16 { id: 2, offset: 2, endian: Little },
} } }
fn main() {}
