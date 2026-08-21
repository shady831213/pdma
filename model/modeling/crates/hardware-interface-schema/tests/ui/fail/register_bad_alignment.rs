use hardware_interface_schema::register_schema;

register_schema! {
    pub register BadAlignment {
        width: 4,
        alignment: 3,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

fn main() {}
