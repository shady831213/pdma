use hardware_interface_schema::record_schema;

record_schema! { pub record Packet { endian: Little, fields: {
    kind: enum(u8) { variants: { A = 1 } },
    payload: union { tag: self.kind, cases: { A { value: u32 } } },
} } }

fn main() {}
