use hw_model_schema::bits_schema;
bits_schema! { pub bits Collision(u8) {
    __: 0..1 => enum { variants: { Value = 0 } },
} }
fn main() {}
