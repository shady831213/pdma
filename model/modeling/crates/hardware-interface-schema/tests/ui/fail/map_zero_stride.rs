use hardware_interface_schema::{register_map, register_schema};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {}, } }
register_map! { pub map Bad { alignment: 4, extent: 32, placements: [
    lane: Word { count: 2, stride: 0 },
] } }
fn main() {}
