use hardware_interface_schema::union_schema;
union_schema! { union Broken { tag: self.kind, cases: { One(A) } } }
fn main() {}
