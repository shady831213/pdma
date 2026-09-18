use crate::{Address, Switch};
use hardware_interface_schema::{record_schema, register_map, register_schema};

record_schema! {
    #[short_description = "Firmware control request"]
    #[description = "Host-produced 16-byte little-endian request, eight-byte aligned. Firmware interprets this independent control queue; DMA hardware never fetches these records."]
    pub record ControlRequest {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Host-selected request identifier echoed unchanged in the response."]
            id: u64,
            #[description = "0: AcquireChannel requests one available DMA channel. 1: ReleaseChannel resets and frees the selected channel. 2: ResetChannel resets the selected channel while retaining its allocation. Other values are invalid."]
            opcode: enum(u32) { variants: { AcquireChannel = 0, ReleaseChannel = 1, ResetChannel = 2 } },
            #[description = "DMA channel index for release or reset; ignored for acquisition."]
            channel: u32,
        },
    }
}

record_schema! {
    #[short_description = "Firmware control response"]
    #[description = "Firmware-produced 16-byte little-endian response, eight-byte aligned. Host reads this independent control queue; hardware stores only bootstrap registers and does not implement channel allocation policy."]
    pub record ControlResponse {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Identifier copied unchanged from the corresponding control request."]
            id: u64,
            #[description = "0: Success. 1: NoResources, no channel is free. 2: InvalidChannel, selected index is outside the DMA. 3: NotAllocated, selected channel is free. 4: PermissionDenied, selected channel belongs to another Function. Other values are invalid."]
            status: enum(u32) { variants: { Success = 0, NoResources = 1, InvalidChannel = 2, NotAllocated = 3, PermissionDenied = 4 } },
            #[description = "Allocated channel on successful acquisition, or the selected channel for release/reset responses. Undefined for failed acquisition."]
            channel: u32,
        },
    }
}

register_map! {
    #[description = "One complete bootstrap and doorbell block per platform Function. Function identity comes from the bank index, never the request payload."]
    pub map FunctionControlRegisters(function_count: usize) {
        alignment: 8,
        placements: [pub functions: ControlRegisters[function_count]],
    }
}

record_schema! {
    #[short_description = "Memory-resident control ring state"]
    #[description = "Software-owned ring geometry and wrapping entry counts. The producer writes producer after storing an entry; the consumer writes consumer after reading an entry. Geometry remains stable while enabled."]
    pub record ControlRingState {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Memory address of the entry buffer."]
            base: u64,
            #[description = "Nonzero power-of-two entry capacity below 2^31, validated by software."]
            capacity: u32,
            #[description = "Wrapping published-entry count, owned by the producer."]
            producer: u32,
            #[description = "Wrapping consumed-entry count, owned by the consumer."]
            consumer: u32,
        },
    }
}

record_schema! {
    #[short_description = "Memory-resident firmware control queue"]
    #[description = "Host allocates and initializes both rings before publishing this record's address through bootstrap. Each software side updates only its owned cursor fields."]
    pub record ControlQueue {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Host-produced and firmware-consumed control requests."]
            command: ControlRingState,
            #[description = "Firmware-produced and Host-consumed control responses."]
            response: ControlRingState,
        },
    }
}

register_schema! {
    #[short_description = "Coalescing notification doorbell"]
    #[description = "Writing one emits a coalescing pulse; writing zero has no effect. The stored value returns to zero after every accepted write. Reads are forbidden and bits 1 through 31 are reserved."]
    pub register Doorbell {
        width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 1,
        fields: {
            #[description = "Write one to notify the peer software domain."]
            pulse: 0..=0,
        },
    }
}

register_map! {
    #[short_description = "Independent firmware control channel"]
    #[description = "Always-on bootstrap and software notification registers outside DMA work windows. Hardware performs no memory operations, ring validation or channel allocation."]
    pub map ControlRegisters {
        alignment: 8,
        placements: [
            #[description = "Host-owned address of the memory-resident ControlQueue. Both domains may read it. Changes are locked after activation; address zero has no special hardware meaning."]
            pub bootstrap: Address,
            #[description = "Host writes one after initializing queue memory. Both domains may read activation. Disabling and bootstrap changes are forbidden after activation; there is no live reset register."]
            pub enable: Switch,
            #[description = "Host-only notification to firmware after publishing requests or freeing response space."]
            pub request: Doorbell,
            #[description = "Firmware-only notification to Host after publishing responses or freeing command space."]
            pub response: Doorbell,
        ],
    }
}
