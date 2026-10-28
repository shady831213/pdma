use hw_model_schema::register_schema;

register_schema! {
    register Broken {
        width: 1,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xff,
        fields: {
            value: 1..9,
        },
    }
}

fn main() {}
