use crate::{Address, Switch, Word};
use hardware_interface_schema::{record_schema, register_map};

/// Always-on firmware control channel, independent of DMA channel windows.
pub const CONTROL_CHANNEL_BASE: u32 = 0xffff_fe00;

record_schema! {
    #[short_description = "Firmware control request"]
    #[description = "Host-produced 16-byte little-endian request, eight-byte aligned. Firmware interprets this independent control queue; DMA hardware never fetches these records."]
    pub record ControlRequest {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Host-selected request identifier echoed unchanged in the response."]
            id: u64,
            #[description = "0: AcquireChannel requests one available DMA channel. 1: ReleaseChannel releases the channel field. Other values are invalid."]
            opcode: enum(u32) { variants: { AcquireChannel = 0, ReleaseChannel = 1 } },
            #[description = "DMA channel index for ReleaseChannel; ignored for AcquireChannel."]
            channel: u32,
        },
    }
}

record_schema! {
    #[short_description = "Firmware control response"]
    #[description = "Firmware-produced 16-byte little-endian response, eight-byte aligned. Host reads this independent control queue; hardware stores only ring registers and does not implement channel allocation policy."]
    pub record ControlResponse {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Identifier copied unchanged from the corresponding control request."]
            id: u64,
            #[description = "0: Success. 1: NoResources, all DMA channels are allocated. 2: InvalidChannel, release index is outside the device. 3: NotAllocated, release index is not allocated. Other values are invalid."]
            status: enum(u32) { variants: { Success = 0, NoResources = 1, InvalidChannel = 2, NotAllocated = 3 } },
            #[description = "Allocated channel on successful acquisition, or the requested release index for release responses. Undefined for failed acquisition."]
            channel: u32,
        },
    }
}

register_map! {
    #[short_description = "Firmware control ring registers"]
    #[description = "One software-to-software queue. Both domains may read every register. Host owns geometry; producer and consumer write permissions depend on command or response placement and are enforced by hardware."]
    pub map ControlRingRegisters {
        alignment: 8,
        placements: [
            #[description = "Host-configured eight-byte-aligned address. Sixteen-byte entries occupy the complete capacity. Locked after activation until subsystem reset."]
            pub base: Address,
            #[description = "Host-configured nonzero power-of-two entry capacity below 2^31. The complete memory extent must fit u64. Locked after activation until subsystem reset."]
            pub capacity: Word,
            #[description = "Wrapping published-entry count. Host writes command producer; firmware writes response producer, only after successful record storage. Active publication cannot exceed free capacity."]
            pub producer: Word,
            #[description = "Wrapping consumed-entry count. Firmware writes command consumer; Host writes response consumer, only after successful record reading. Active consumption cannot exceed published entries."]
            pub consumer: Word,
        ],
    }
}

register_map! {
    #[short_description = "Independent firmware control channel"]
    #[description = "Always-on Host/firmware control register composition outside DMA work windows. Hardware owns authoritative pointers but performs no memory operations or channel allocation. Pointer writes require activation, including unchanged zero; wrong-domain writes are always rejected."]
    pub map ControlRegisters {
        alignment: 8,
        placements: [
            #[description = "Host-produced and firmware-consumed control requests."]
            pub command: ControlRingRegisters,
            #[description = "Firmware-produced and Host-consumed control responses."]
            pub response: ControlRingRegisters,
            #[description = "Host writes one after configuring both rings. Both geometries are validated atomically before activation. Activation locks geometry and prevents disabling until subsystem reset; there is no live reset register."]
            pub enable: Switch,
        ],
    }
}
