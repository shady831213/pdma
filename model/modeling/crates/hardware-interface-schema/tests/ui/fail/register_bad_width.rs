use hardware_interface_schema::register_schema;
register_schema! { pub registers Bad {
    value: { id: 1, name: "value", offset: 0, width: 3, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
} }
fn main() {}
