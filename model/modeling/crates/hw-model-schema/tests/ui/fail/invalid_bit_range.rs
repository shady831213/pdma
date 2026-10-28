use hw_model_schema::register_schema;
register_schema! { pub register Bad {
    width: 1, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xff,
    fields: { wide: 1..9, },
} }

fn main() {}
