use hardware_interface_schema::register_schema;

register_schema! {
    pub registers LegacyRegisters {
        value: {
            id: 1,
            name: "VALUE",
            offset: 0,
            width: 4,
            access: ReadWrite,
            reset: Zero(0),
            writable_mask: 0xffff_ffff
        },
    }
}

fn main() {}
