use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    pub register Word {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_map! {
    pub map BadOffset {
        alignment: 4,
        placements: [word: Word { offset: 2 }],
    }
}

fn main() {}
