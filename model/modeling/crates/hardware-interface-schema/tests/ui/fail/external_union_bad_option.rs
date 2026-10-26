use hardware_interface_schema::union_schema;
union_schema! { union Packet { cases: { Item(Item) }, codec: inferred } }
fn main() {}
