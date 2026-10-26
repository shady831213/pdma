use hardware_interface_schema::record_schema;

record_schema! {
    record Broken {
        endian: Little,
        fields: {
            opcode: enum(u8) { variants: { First = 1 } },
            payload: tagged_union {
                tag: self.opcode.extra,
                cases: { First { value: u8 } },
            },
        },
    }
}

fn main() {}
