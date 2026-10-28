use hw_model_schema::{record_schema, union_schema};
record_schema! { record Leaf { endian: Little, fields: { value: u16 } } }
union_schema! { union Raw { cases: { Leaf(Leaf) } } }
union_schema! { union Outer { cases: { Raw(Raw) } } }
fn main() {}
