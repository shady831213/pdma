use hardware_interface_schema::{record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
fn main() {}
