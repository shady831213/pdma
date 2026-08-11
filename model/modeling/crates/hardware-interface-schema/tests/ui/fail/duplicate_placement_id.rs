use hardware_interface_schema::{field_schema, record_schema};

field_schema! { pub field Byte { scalar: u8, endian: Little } }
record_schema! { pub record Bad {
    size: 2, alignment: 1,
    fields: {
        first: Byte { id: 1, offset: 0 },
        second: Byte { id: 1, offset: 1 },
    }, records: {},
} }

fn main() {}
