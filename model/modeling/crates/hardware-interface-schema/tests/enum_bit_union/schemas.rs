use hardware_interface_schema::record_schema;

record_schema! {
    pub record Packet {
        endian: Little,
        fields: {
            prefix: u16,
            flags: bits(u16) { fields: {
                ready: 0..=0,
                kind: 3..=4 => enum { variants: { Small = 1, Large = 2 } },
                priority: 5..=7,
            } },
            payload: tagged_union {
                tag: self.flags.kind,
                cases: { Small { first: u16 }, Large { first: u16, second: u16 } },
            },
        },
    }
}

/// Qualified reusable header fixture.
pub mod named {
    use hardware_interface_schema::bits_schema;

    bits_schema! {
        pub bits Header(u16) {
            ready: 0..=0,
            priority: 1..=3,
            kind: 8..=9 => enum { variants: { Small = 1, Large = 2 } },
        }
    }
}

record_schema! {
    pub record Pair {
        endian: Little,
        fields: {
            tags: bits(u8) { fields: {
                left: 0..=0 => enum { variants: { First = 0, Second = 1 } },
                right: 2..=2 => enum { variants: { First = 0, Second = 1 } },
                ready: 7..=7,
            } },
            left: tagged_union { tag: self.tags.left,
                cases: { First { value: u8 }, Second { value: u8 } },
            },
            right: tagged_union { tag: self.tags.right,
                cases: { First { value: u8 }, Second { value: u8 } },
            },
        },
    }
}

record_schema! { pub record Small { endian: Big, fields: { first: u16 } } }
record_schema! { pub record Large { endian: Big, fields: { first: u16, second: u16 } } }

record_schema! {
    pub record NamedPacket {
        endian: Big,
        fields: {
            prefix: u16,
            header: bits(named::Header),
            payload: tagged_union {
                tag: self.header.kind,
                cases: { Small(Small), Large(Large) },
            },
        },
    }
}
