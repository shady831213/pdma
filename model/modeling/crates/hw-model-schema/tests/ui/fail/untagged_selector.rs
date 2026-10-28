use hw_model_schema::union_schema;
union_schema! { union Broken { tag: self.kind, cases: { One(A) } } }
fn main() {}
