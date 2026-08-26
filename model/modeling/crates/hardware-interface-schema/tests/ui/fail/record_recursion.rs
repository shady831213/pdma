use hardware_interface_schema::record_schema;

record_schema! { pub record Recursive {
    alignment: 4, endian: Little,
    fields: { child: Recursive, },
} }

fn main() {}
