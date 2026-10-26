use hardware_interface_schema::union_schema;

union_schema! { pub union Packet {
    cases: { A(Record) },
    tag: self.kind,
} }

fn main() {}
