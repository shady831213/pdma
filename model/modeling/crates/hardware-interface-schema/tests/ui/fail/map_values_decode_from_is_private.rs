mod generated {
    use hardware_interface_schema::{register_map, register_schema};

    register_schema! {
        pub register Word {
            width: 4,
            endian: Little,
            access: ReadOnly,
            reset: Zero(0),
            writable_mask: 0,
            fields: {},
        }
    }

    register_map! {
        pub map DeviceMap {
            alignment: 4,
            placements: [word: Word],
        }
    }
}

fn main() {
    let bytes = [0; 4];
    let _ = generated::DeviceMapValues::decode_from(&bytes);
}
