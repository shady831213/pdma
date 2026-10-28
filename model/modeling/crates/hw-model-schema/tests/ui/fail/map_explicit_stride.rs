use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_map! { pub map ExplicitStride {
    alignment: 4, placements: [children: Word[2] { stride: 8 }],
} }
fn main() {}
