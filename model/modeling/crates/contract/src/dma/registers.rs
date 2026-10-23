mod channel;

pub use channel::{
    ChannelAdmin, ChannelAdminRaw, ChannelAdminRegisterId, ChannelAdminValues, ChannelAdminView,
    ChannelAdminViewMut, ChannelBlock, ChannelBlockRaw, ChannelBlockRegisterId, ChannelBlockValues,
    ChannelBlockView, ChannelBlockViewMut, ChannelConfiguration, ChannelConfigurationRaw,
    ChannelConfigurationRegisterId, ChannelConfigurationValues, ChannelConfigurationView,
    ChannelConfigurationViewMut, ChannelFatal, ChannelFatalCode, ChannelFatalCodeRaw,
    ChannelFatalFieldId, ChannelFatalRaw, ChannelFatalValues, ChannelFatalView,
    ChannelFatalViewMut, ChannelInterrupts, ChannelInterruptsRaw, ChannelInterruptsRegisterId,
    ChannelInterruptsValues, ChannelInterruptsView, ChannelInterruptsViewMut, ChannelIrqCause,
    ChannelIrqCauseFieldId, ChannelIrqCauseRaw, ChannelIrqCauseValues, ChannelIrqCauseView,
    ChannelIrqCauseViewMut, ChannelIrqMask, ChannelIrqMaskFieldId, ChannelIrqMaskRaw,
    ChannelIrqMaskValues, ChannelIrqMaskView, ChannelIrqMaskViewMut, ChannelRecovery,
    ChannelRecoveryRaw, ChannelRecoveryRegisterId, ChannelRecoveryValues, ChannelRecoveryView,
    ChannelRecoveryViewMut, ChannelRegisters, ChannelRegistersRaw, ChannelRegistersRegisterId,
    ChannelRegistersValues, ChannelRegistersView, ChannelRegistersViewMut, CommandRegisters,
    CommandRegistersRaw, CommandRegistersRegisterId, CommandRegistersValues, CommandRegistersView,
    CommandRegistersViewMut, EventRegisters, EventRegistersRaw, EventRegistersRegisterId,
    EventRegistersValues, EventRegistersView, EventRegistersViewMut, FaultStatus,
    FaultStatusFieldId, FaultStatusRaw, FaultStatusValues, FaultStatusView, FaultStatusViewMut,
    Resume, ResumeFieldId, ResumeRaw, ResumeValues, ResumeView, ResumeViewMut, TrapAck,
    TrapAckFieldId, TrapAckRaw, TrapAckValues, TrapAckView, TrapAckViewMut,
};

use crate::register::Switch;
use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    #[short_description = "Hardware channel count"]
    #[description = "Read-only count supplied by this hardware instance. Hardware initializes the value after construction and preserves it across device reset."]
    pub register ChannelCount {
        width: 4, endian: Little, access: ReadOnly,
        reset: PostReset(0), writable_mask: 0, fields: {},
    }
}

register_schema! {
    #[short_description = "Hardware reset request and busy state"]
    #[description = "Writing one to bit zero requests reset of the owning hardware scope. Reading one means reset is busy; only hardware clears it to zero after draining all prior command-ring reads, SG-list reads, payload reads/writes and event-ring writes, invalidating retained work and clearing configuration, cursors, errors and device-owned notification state. Once zero is observed, no further memory access can belong to pre-reset work in that scope. Writing zero has no effect, and repeated requests while busy coalesce. Configuration writes are rejected while reset is busy. External memory is neither cleared nor rolled back; platform-pending notifications remain claimable. Software must keep affected memory valid until completion, even if it cancels or times out its wait. Other bits are reserved."]
    pub register Reset {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1,
        fields: {
            #[short_description = "Write one to request reset; read one while busy and zero after hardware cleanup"]
            #[description = "Write one to request reset; read one while busy and zero after hardware cleanup. Writing zero cannot clear a pending request."]
            request: 0..=0,
        },
    }
}

register_schema! {
    #[short_description = "Dma fatal state"]
    #[description = "A device-wide hardware fatal stops all channels. Only device reset clears it; channel reset does not. This is a hardware fatal input, not a software memory-management error."]
    pub register DmaFatal {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: {
            #[short_description = "Zero is healthy; Internal is a device-wide unrecoverable hardware fatal"]
            #[description = "Zero is healthy; Internal is a device-wide unrecoverable hardware fatal."]
            code: 0..=31 => enum { variants: { None = 0, Internal = 1 } },
        },
    }
}

register_map! {
    #[short_description = "DMA discovery"]
    #[description = "Read-only hardware instance configuration, independent of privileged DMA controls."]
    pub map DmaDiscovery {
        alignment: 4,
        placements: [
            #[short_description = "Implemented channel count"]
            #[description = "Number of implemented channels, preserved across DMA reset."]
            pub channel_num: ChannelCount,
            #[short_description = "Immutable native or exclusive atomic operation support by word width"]
            #[description = "Immutable native or exclusive atomic operation support by word width."]
            pub capabilities: Capabilities,
        ],
    }
}

register_map! {
    #[short_description = "Dma-wide controls"]
    #[description = "Owns the device fatal input, its interrupt enable and whole-device reset. Channel causes and enables remain channel-owned."]
    pub map DmaControl {
        alignment: 4,
        placements: [
            #[short_description = "Enables notification of device fatal independently of channel IRQ enable"]
            #[description = "Enables notification of device fatal independently of channel IRQ enable."]
            pub irq_enable: Switch,
            #[short_description = "Dma fatal cause; read-only and sticky until device reset"]
            #[description = "Dma fatal cause; read-only and sticky until device reset."]
            pub fatal: DmaFatal,
            #[short_description = "Requests reset of device controls and every channel, including retained execution"]
            #[description = "Requests reset of device controls and every channel, including retained execution. Reads busy until all affected memory accesses drain and cleanup completes. Hardware channel count is preserved."]
            pub reset: Reset,
        ],
    }
}

register_map! {
    #[short_description = "DMA administration aperture"]
    #[description = "Contiguous privileged DMA controls and per-channel reset registers, separate from discovery and data apertures."]
    pub map DmaAdmin(channel_count: usize) {
        alignment: 4,
        placements: [
            #[short_description = "DMA-wide administration"]
            #[description = "Global interrupt control, fatal state and whole-DMA reset."]
            pub control: DmaControl,
            #[short_description = "Channel administration bank"]
            #[description = "One privileged recovery block for each implemented channel."]
            pub channels: ChannelAdmin[channel_count],
        ],
    }
}

register_map! {
    #[short_description = "PDMA register apertures"]
    #[description = "Read-only discovery followed by contiguous administration and page-aligned channel data apertures. Layout gaps are reserved, not registers."]
    pub map DmaRegisters(channel_count: usize) {
        alignment: 4096,
        placements: [
            #[short_description = "Hardware discovery aperture"]
            #[description = "Read-only DMA configuration visible independently of administrative permissions."]
            pub discovery: DmaDiscovery,
            #[short_description = "Privileged DMA administration"]
            #[description = "Complete global and per-channel administration aperture, hidden from managed Host software."]
            pub admin: DmaAdmin(channel_count),
            #[short_description = "Independent channel data interfaces"]
            #[description = "Page-aligned complete channel blocks containing rings, configuration, fatal failures and notifications. Unused bytes are reserved to their owning block."]
            pub channels: ChannelBlock[channel_count],
        ],
    }
}

register_schema! {
    #[short_description = "Immutable hardware RMW capabilities, preserved across reset"]
    #[description = "Immutable hardware RMW capabilities, preserved across reset. Wait and Fence do not require these bits."]
    pub register Capabilities {
        width: 4, endian: Little, access: ReadOnly, reset: PostReset(0), writable_mask: 0,
        fields: {
            #[short_description = "32-bit atomic and semaphore operations are supported"]
            #[description = "32-bit atomic and semaphore operations are supported."]
            atomic32: 0..=0,
            #[short_description = "64-bit atomic and semaphore operations are supported"]
            #[description = "64-bit atomic and semaphore operations are supported."]
            atomic64: 1..=1,
        },
    }
}
