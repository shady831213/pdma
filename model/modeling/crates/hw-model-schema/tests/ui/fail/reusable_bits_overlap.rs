use hw_model_schema::bits_schema;

bits_schema! {
    pub bits OverlappingBits(u8) {
        first: 0..2,
        second: 1..3,
    }
}

fn main() {}
