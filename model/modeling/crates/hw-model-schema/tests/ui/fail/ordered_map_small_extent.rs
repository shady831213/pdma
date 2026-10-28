use hw_model_schema::{register_map, register_schema};

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
    pub map SmallExtent {
        alignment: 4,
        extent: 3,
        placements: [word: Word],
    }
}

fn main() {}
