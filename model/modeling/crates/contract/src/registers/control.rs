use super::{ChannelCount, Switch};
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
    #[short_description = "Hardware reset request and busy state"]
    #[description = "Writing one to bit zero requests reset of the owning hardware scope. Reading one means reset is busy; only hardware clears it to zero after draining all prior command-ring reads, SG-list reads, payload reads/writes and response-ring writes, invalidating retained work and clearing configuration, cursors, errors and device-owned notification state. Once zero is observed, no further memory access can belong to pre-reset work in that scope. Writing zero has no effect, and repeated requests while busy coalesce. Configuration writes are rejected while reset is busy. External memory is neither cleared nor rolled back; platform-pending notifications remain claimable. Software must keep affected memory valid until completion, even if it cancels or times out its wait. Other bits are reserved."]
    pub register Reset {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1,
        fields: {
            #[description = "Write one to request reset; read one while busy and zero after hardware cleanup. Writing zero cannot clear a pending request."]
            request: 0..=0,
        },
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
            #[short_description = "Implemented channel count"]
            #[description = "Number of physical channel register blocks. Software discovers this read-only hardware configuration before selecting a channel; device reset preserves it."]
            pub channel_num: ChannelCount,
            #[description = "Enables notification of device fatal independently of channel IRQ enable."]
            pub irq_enable: Switch,
            #[description = "Device fatal cause; read-only and sticky until device reset."]
            pub fault: DeviceFault,
            #[description = "Requests reset of device controls and every channel, including retained execution. Reads busy until all affected memory accesses drain and cleanup completes. Hardware channel count is preserved."]
            pub reset: Reset,
        ],
    }
}
