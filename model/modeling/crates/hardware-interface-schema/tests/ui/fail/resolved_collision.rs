use hardware_interface_schema::{field_schema, record_schema};

field_schema! { pub field Word { scalar: u32, endian: Little } }
record_schema! { pub record Child {
    size: 4, alignment: 4,
    fields: { value: Word { id: 1, offset: 0 }, }, records: {},
} }
record_schema! { pub record Bad {
    size: 8, alignment: 4,
    fields: { value: Word { id: 1, offset: 0 }, },
    records: { child: Child { id: 2, offset: 0 }, },
} }

fn main() {}
