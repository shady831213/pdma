use hw_model_schema::record_schema;

record_schema! { pub record Bad {
    alignment: 4, endian: Little,
    fields: { value: MissingDefinition, },
} }

fn main() {}
