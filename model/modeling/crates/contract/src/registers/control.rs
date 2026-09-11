use super::Switch;
use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    #[short_description = "Channel interrupt causes"]
    #[description = "Hardware causes visible regardless of masks. Reading does not acknowledge notifications. Non-empty follows occupancy, IOC clears on consumption, and error/fatal require channel reset."]
    pub register ChannelIrqCause {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: {
            #[description = "The response ring currently contains unread entries."]
            response_non_empty: 0..=0,
            #[description = "At least one published IOC response remains unconsumed."]
            ioc: 1..=1,
            #[description = "An error response was published; processing is stopped until reset."]
            response_error: 2..=2,
            #[description = "The channel fault register contains a fatal error."]
            channel_fatal: 3..=3,
        },
    }
}

register_schema! {
    #[short_description = "Channel response notification masks"]
    #[description = "One suppresses that source's future notifications without clearing causes. Unmask does not replay past events; software must check queue and error state before waiting. Error and fatal notifications bypass these two masks."]
    pub register ChannelIrqMask {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 3,
        fields: {
            #[description = "Mask notifications on response empty-to-non-empty transitions."]
            response_non_empty: 0..=0,
            #[description = "Mask notifications on publication of IOC responses."]
            ioc: 1..=1,
        },
    }
}

register_schema! {
    #[short_description = "Hardware reset request"]
    #[description = "Writing bit zero resets the owning hardware scope to disabled and unconfigured. Other bits are reserved; writing zero has no effect. External memory is neither cleared nor rolled back."]
    pub register Reset {
        width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 1,
        fields: { request: 0..=0 },
    }
}

register_schema! {
    #[short_description = "Device fatal state"]
    #[description = "A device-wide hardware fault stops all channels. Only device reset clears it; channel reset does not. This is a hardware fault input, not a software memory-management error."]
    pub register DeviceFault {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: {
            #[description = "Zero is healthy; Internal is a device-wide unrecoverable hardware fault."]
            code: 0..=31 => enum { variants: { None = 0, Internal = 1 } },
        },
    }
}

register_map! {
    #[short_description = "Device-wide controls"]
    #[description = "Owns the device fatal input, its interrupt enable and whole-device reset. Channel causes and enables remain channel-owned."]
    pub map DeviceControl {
        alignment: 4,
        placements: [
            #[description = "Enables notification of device fatal independently of channel IRQ enable."]
            pub irq_enable: Switch,
            #[description = "Device fatal cause; read-only and sticky until device reset."]
            pub fault: DeviceFault,
            #[description = "Resets device controls and every channel, including retained execution."]
            pub reset: Reset,
        ],
    }
}
