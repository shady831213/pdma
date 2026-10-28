use hw_model_schema::union_schema;
union_schema! { union Packet { cases: { Item(Item) }, codec: inferred } }
fn main() {}
