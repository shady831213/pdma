use hardware_interface_schema::register_map;
register_map! { pub map Bad { size: 4, fixed: {
    value: { id: 1, name: "value", offset: 2, width: 4, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
}, banks: {} } }
fn main() {}
