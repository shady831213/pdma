use hardware_interface_schema::{field_schema, record_schema};

field_schema! { pub field Word { scalar: u32, endian: Little } }

record_schema! { pub record Bad {
    size: 8, alignment: 4,
    fields: {
        foo: Word { id: 1, offset: 0 },
        FOO: Word { id: 2, offset: 4 },
    },
} }

fn main() {}
