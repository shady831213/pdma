use hw_model_schema::record_schema;

record_schema! {
    pub record Bad {
        endian: Little,
        fields: { state: bits(u8) { fields: { code: 0..2 => enum { variants: {} } } } },
    }
}

fn main() {}
