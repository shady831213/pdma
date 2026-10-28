use hw_model_schema::register_map;
register_map! { pub map Expression(channels: usize) {
    alignment: 4, placements: [children: Missing[channels + 1]],
} }
fn main() {}
