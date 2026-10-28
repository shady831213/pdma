use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_map! { pub map Block {
    alignment: 8, extent: 12,
    placements: [control: Word, status: Word { offset: 8 }],
} }
register_map! { pub map Overlap {
    alignment: 8, placements: [blocks: Block[2], intruder: Word { offset: 12 }],
} }
fn main() {}
