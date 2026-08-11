use hardware_interface_schema::{field_schema, record_schema};

field_schema! { pub field Byte { scalar: u8, endian: Little } }
record_schema! { pub record Child {
    size: 1, alignment: 1,
    fields: { value: Byte { id: 1, offset: 0 }, }, records: {},
} }
record_schema! { pub record Bad {
    size: 2, alignment: 1,
    fields: { child: Byte { id: 1, offset: 0 }, },
    records: { child: Child { id: 2, offset: 1 }, },
} }

fn main() {}
