use hardware_interface_schema::union_schema;
union_schema! { union Broken { cases: { Same(A), Same(B) } } }
fn main() {}
