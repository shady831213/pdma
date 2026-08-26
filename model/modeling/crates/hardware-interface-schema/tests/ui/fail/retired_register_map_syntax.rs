use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    pub register Word {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

register_map! {
    pub map LegacyMap {
        size: 4,
        fixed: {
            value: Word { id: 1, name: "VALUE", offset: 0 },
        },
        banks: {}
    }
}

fn main() {}
