use hw_model_schema::record_schema;

record_schema! {
    #[derive(Clone)]
    record Packet { endian: Little, fields: { value: u8 } }
}

fn main() {}
