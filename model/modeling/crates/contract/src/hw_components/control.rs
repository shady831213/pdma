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
            #[description = "PF programs Default/PASID attributes before enable. Frozen with bootstrap after enable and independent of every context table slot."]
            pub attributes: ContextAttributeRegisters,
        ],
    }
}
