use hw_model_schema::record_schema;

record_schema! { pub record Recursive {
    alignment: 4, endian: Little,
    fields: { child: Recursive, },
} }

fn main() {}
