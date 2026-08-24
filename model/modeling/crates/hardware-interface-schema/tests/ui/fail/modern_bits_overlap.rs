use hardware_interface_schema::record_schema;

record_schema! {
    record Broken {
        endian: Little,
        fields: {
            value: bits(u16) {
                fields: {
                    first: 0..5,
                    second: 4..8,
                },
            },
        },
    }
}

fn main() {}
