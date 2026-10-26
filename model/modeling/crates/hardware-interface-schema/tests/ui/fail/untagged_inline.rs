use hardware_interface_schema::union_schema;
union_schema! { union Broken { cases: { One { value: u32 } } } }
fn main() {}
