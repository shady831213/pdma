use hardware_interface_schema::record_schema;
record_schema! { pub record Bad { alignment: 1, endian: Little, fields: {
    same: u16,
    same: u16,
} } }
fn main() {}
