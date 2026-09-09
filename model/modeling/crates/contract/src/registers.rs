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

register_schema! {
    pub register ReadWord {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_schema! {
    pub register Address {
        width: 8,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff_ffff_ffff,
        fields: {},
    }
}

register_schema! {
    pub register Switch {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 1,
        fields: { enabled: 0..1 },
    }
}

register_map! {
    pub map CommandRegisters {
        alignment: 8,
        placements: [
            pub base: Address,
            pub capacity: Word,
            pub producer: Word,
            pub consumer: ReadWord,
        ],
    }
}

register_map! {
    pub map ResponseRegisters {
        alignment: 8,
        placements: [
            pub base: Address,
            pub capacity: Word,
            pub producer: ReadWord,
            pub consumer: Word,
        ],
    }
}

register_map! {
    pub map ChannelRegisters {
        alignment: 4,
        placements: [
            pub enable: Switch,
            pub irq_enable: Switch,
            pub fault: ReadWord,
        ],
    }
}

register_map! {
    pub map DeviceRegisters {
        alignment: 8,
        placements: [
            pub command: CommandRegisters,
            pub response: ResponseRegisters,
            pub channel: ChannelRegisters,
        ],
    }
}
