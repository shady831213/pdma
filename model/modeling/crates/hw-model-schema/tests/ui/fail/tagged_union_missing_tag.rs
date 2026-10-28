use hw_model_schema::record_schema;

record_schema! { pub record Packet { endian: Little, fields: {
    payload: tagged_union { cases: { A { value: u32 } } },
} } }

fn main() {}
