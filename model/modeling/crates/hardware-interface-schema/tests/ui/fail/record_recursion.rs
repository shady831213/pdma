use hardware_interface_schema::record_schema;

record_schema! { pub record Recursive {
    size: 4, alignment: 4, fields: {},
    records: { child: Recursive { id: 1, offset: 0 }, },
} }

fn main() {}
