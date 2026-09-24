use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    #[short_description = "Firmware boot report"]
    #[description = "Firmware publishes state and error atomically. Host may only read this register. Reset restores Booting with no error; DMA resets do not reset firmware. This register does not grant DMA access."]
    pub register FirmwareBoot {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0x0001_0003,
        fields: {
            #[description = "Firmware initializes after subsystem reset release; Ready means initialization completed, not that any particular DMA window is open."]
            state: 0..=1 => enum {
                variants: {
                    Booting = 0,
                    Ready = 1,
                    Failed = 2,
                },
            },
            #[description = "Initialization failure reported by firmware. A stuck CPU cannot report failure; Host must bound its wait. Runtime failures are not automatically converted into this field."]
            error: 16..=16 => enum {
                variants: {
                    None = 0,
                    WindowSetup = 1,
                },
            },
        },
    }
}

register_map! {
    #[short_description = "Firmware lifecycle status"]
    #[description = "Always accessible independently of DMA window permissions. Firmware can read/write; hardware denies all Host writes."]
    pub map FirmwareRegisters {
        alignment: 4,
        placements: [
            #[description = "One atomic firmware boot state and error report."]
            pub boot: FirmwareBoot,
        ],
    }
}
