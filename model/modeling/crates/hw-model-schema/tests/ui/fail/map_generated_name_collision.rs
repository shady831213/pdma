use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word {
    width: 1, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0,
    fields: {},
} }
register_map! { pub map Collision {
    alignment: 1, placements: [pub foo_bar: Word, pub foo__bar: Word],
} }
fn main() {}
