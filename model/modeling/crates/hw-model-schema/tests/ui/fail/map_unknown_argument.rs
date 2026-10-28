use hw_model_schema::register_map;
register_map! { pub map Argument(channels: usize) {
    alignment: 4, placements: [children: Missing(lanes)[channels]],
} }
fn main() {}
