use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! {
    bits Header(u8) { kind: 0..=1 }
}

record_schema! {
    record Packet {
        endian: Little,
        fields: {
            flags: bits(Header),
            payload: tagged_union { tag: self.flags.kind, cases: { First { value: u8 } } },
        },
    }
}

fn main() {}
