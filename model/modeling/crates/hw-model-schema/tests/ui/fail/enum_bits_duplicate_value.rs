use hw_model_schema::register_schema;

register_schema! {
    pub register Bad {
        width: 1, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 3,
        fields: { code: 0..2 => enum { variants: { First = 0, Second = 0 } } },
    }
}

fn main() {}
