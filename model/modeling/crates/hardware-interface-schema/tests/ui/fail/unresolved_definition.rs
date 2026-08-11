use hardware_interface_schema::record_schema;

record_schema! { pub record Bad {
    size: 4, alignment: 4,
    fields: { value: MissingDefinition { id: 1, offset: 0 }, },
    records: {},
} }

fn main() {}
