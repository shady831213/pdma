use hardware_interface_schema::register_map;
register_map! { pub map Duplicate(channels: usize, channels: usize) {
    alignment: 4, placements: [children: Missing[channels]],
} }
fn main() {}
