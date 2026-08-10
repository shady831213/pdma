use hardware_interface_schema::register_map;
register_map! { pub map Bad { size: 32, fixed: {}, banks: {
    lane: { count: 2, base: 0, stride: 0, members: {
        value: { id: 1, name: "value", offset: 0, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
    } },
} } }
fn main() {}
