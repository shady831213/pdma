use hw_model_schema::record_schema;

record_schema! {
    record Broken {
        endian: Little,
        fields: {
            opcode: enum(u8) { variants: { First = 1, Second } },
            payload: tagged_union {
                tag: self.opcode,
                cases: { First { value: u8 } },
            },
        },
    }
}

fn main() {}
