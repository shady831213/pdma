use hardware_interface_register::RegisterImage;
use his::{register_map, register_schema};

register_schema! {
    pub register CompactResponse {
        width: 2, alignment: 2, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 0xffff, fields: {},
    }
}

register_map! {
    pub map CompactMap {
        alignment: 2,
        extent: 2,
        placements: [response: CompactResponse],
    }
}

fn main() {
    let _invalid: Option<RegisterImage<CompactMap, CompactResponseRaw>> = None;
}
