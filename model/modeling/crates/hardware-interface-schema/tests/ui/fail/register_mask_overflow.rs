use hardware_interface_schema::register_schema;
register_schema! { pub registers Bad {
    value: { id: 1, name: "value", offset: 0, width: 1, access: ReadWrite,
        reset: Zero(0), writable_mask: 0x100 },
} }
fn main() {}
