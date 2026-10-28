use hw_model_schema::record_schema;

record_schema! {
    pub record LegacyRecord {
        size: 4,
        alignment: 4,
        fields: {
            value: u32 { id: 1, offset: 0, endian: Little },
        }
    }
}

fn main() {}
