use hardware_interface_schema::{register_map, register_schema};
register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_map! { pub map Block {
    alignment: 4, extent: 16,
    placements: [control: Word, status: Word { offset: 8 }],
} }
register_map! { pub map Overlap {
    alignment: 4, placements: [blocks: Block[2], intruder: Word { offset: 4 }],
} }
fn main() {}
