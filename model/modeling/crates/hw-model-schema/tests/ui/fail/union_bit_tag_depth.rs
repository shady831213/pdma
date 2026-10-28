use hw_model_schema::record_schema;

record_schema! {
    record Packet {
        endian: Little,
        fields: {
            flags: bits(u8) { fields: {
                kind: 0..=0 => enum { variants: { Item = 0 } },
            } },
            payload: tagged_union { tag: self.flags.kind.extra, cases: { Item { value: u8 } } },
        },
    }
}

fn main() {}
