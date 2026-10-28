use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {}, } }
register_map! { pub map Bad { alignment: 4, placements: [first: Word, second: Word], access: [
    group.same => first,
    group.same => second,
] } }
fn main() {}
