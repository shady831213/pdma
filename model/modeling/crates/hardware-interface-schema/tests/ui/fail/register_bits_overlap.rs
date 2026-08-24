use hardware_interface_schema::register_schema;

register_schema! {
    register Broken {
        width: 2,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xff,
        fields: {
            first: 0..5,
            second: 4..8,
        },
    }
}

fn main() {}
