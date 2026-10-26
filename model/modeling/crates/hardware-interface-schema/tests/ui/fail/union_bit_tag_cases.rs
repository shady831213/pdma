use hardware_interface_schema::record_schema;

record_schema! {
    record Packet {
        endian: Little,
        fields: {
            flags: bits(u8) { fields: {
                kind: 0..=1 => enum { variants: { First = 0, Second = 1 } },
            } },
            payload: union { tag: self.flags.kind, cases: { First { value: u8 } } },
        },
    }
}

fn main() {}
