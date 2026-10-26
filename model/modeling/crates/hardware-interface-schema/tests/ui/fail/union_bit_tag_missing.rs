use hardware_interface_schema::record_schema;

record_schema! {
    record Packet {
        endian: Little,
        fields: {
            flags: bits(u8) { fields: { enabled: 0..=0 } },
            payload: tagged_union { tag: self.flags.kind, cases: { Item { value: u8 } } },
        },
    }
}

fn main() {}
