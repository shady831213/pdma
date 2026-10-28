use hw_model_schema::bits_schema;

bits_schema! {
    #[description = "First"]
    #[description = "Second"]
    bits Flags(u8) { enabled: 0..1 }
}

fn main() {}
