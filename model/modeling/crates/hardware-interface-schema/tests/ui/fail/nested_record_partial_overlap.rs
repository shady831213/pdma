use hardware_interface_schema::record_schema;

record_schema! { pub record Child {
    size: 4, alignment: 4,
    fields: { value: u32 { id: 1, offset: 0, endian: Little }, },
} }

record_schema! { pub record Bad {
    size: 8, alignment: 4,
    fields: { value: u32 { id: 1, offset: 0, endian: Little }, },
    records: { child: Child { id: 2, offset: 2 }, },
} }

fn main() {}
