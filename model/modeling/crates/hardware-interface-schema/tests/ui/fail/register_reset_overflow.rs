use hardware_interface_schema::register_schema;
register_schema! { pub registers Bad {
    value: { id: 1, name: "value", offset: 0, width: 1, access: ReadOnly,
        reset: Constant(0x100), writable_mask: 0 },
} }
fn main() {}
