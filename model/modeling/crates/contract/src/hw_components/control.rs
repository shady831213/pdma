use crate::hw_components::context::ContextAttributeRegisters;
use crate::register::{Address, Switch};
use hardware_interface_schema::{register_map, register_schema};

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

register_schema! {
    #[short_description = "Retained event delivery error"]
    #[description = "Firmware latches first delivery failure and clears it after explicit event recovery. PF reads only; hardware performs no queue inspection."]
    pub register EventRingError {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 3,
        fields: {
            #[description = "0: None. 1: Overflow. 2: AccessError. 3: InvalidRing. Nonzero means event history is incomplete and the producer is stopped."]
            code: 0..=1 => enum { variants: { None = 0, Overflow = 1, AccessError = 2, InvalidRing = 3 } },
        },
    }
}

register_map! {
    #[short_description = "Firmware-to-PF notifications"]
    #[description = "Independent response and event doorbells with retained event delivery status. Firmware owns writes; PF can read only event_error."]
    pub map ControlNotificationRegisters {
        alignment: 4,
        placements: [
            #[description = "Firmware-only notification to Host after publishing responses or freeing command space."]
            pub response: Doorbell,
            #[description = "Firmware-only event/error notification to PF, independent of responses."]
            pub event: Doorbell,
            #[description = "Firmware-owned event delivery error; PF can only read. DMA reset does not clear it."]
            pub event_error: EventRingError,
        ],
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
            #[description = "PF programs Default/PASID attributes before enable. Frozen with bootstrap after enable and independent of every context table slot."]
            pub attributes: ContextAttributeRegisters,
            #[description = "Host-only notification to firmware after publishing requests or freeing response space."]
            pub request: Doorbell,
            #[description = "Firmware-to-PF response and event reporting, independent of request admission."]
            pub notification: ControlNotificationRegisters,
        ],
    }
}
