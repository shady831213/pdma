use hw_model_schema::register_schema;

register_schema! {
    register Control {
        width: 1, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1,
        fields: { #[short_description = 42] enabled: 0..1 },
    }
}

fn main() {}
