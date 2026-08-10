use hardware_interface_schema::register_map;
register_map! { pub map Bad { size: 32, fixed: {
    fixed: { id: 1, name: "fixed", offset: 8, width: 4, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
}, banks: {
    lane: { count: 2, base: 0, stride: 8, members: {
        value: { id: 2, name: "value", offset: 0, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
    } },
} } }
fn main() {}
