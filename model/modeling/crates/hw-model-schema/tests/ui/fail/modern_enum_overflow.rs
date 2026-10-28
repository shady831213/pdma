use hw_model_schema::record_schema;

record_schema! {
    record Broken {
        endian: Little,
        fields: {
            value: enum(u8) {
                variants: { First = 256 },
            },
        },
    }
}

fn main() {}
