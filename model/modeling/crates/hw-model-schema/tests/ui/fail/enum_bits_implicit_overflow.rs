use hw_model_schema::bits_schema;

bits_schema! {
    pub bits Bad(u8) {
        code: 0..2 => enum { variants: { Last = 3, Overflow } },
    }
}

fn main() {}
