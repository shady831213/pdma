use crate::register::{ReadWord, Switch, Word};
use hardware_interface_schema::{register_map, register_schema};

impl ContextAttributeRegistersValues {
    /// Decodes and validates one complete address-space configuration.
    pub fn attributes(
        &self,
    ) -> Result<crate::memory::ContextAttributes, crate::memory::InvalidContextAttributes> {
        let address_space =
            crate::memory::ContextAttributesAddressSpace::from_raw(self.address_space.raw())
                .map_err(|_| crate::memory::InvalidContextAttributes)?;
        let attributes = crate::memory::ContextAttributes {
            address_space,
            pasid: self.pasid.raw(),
        };
        attributes.validate()?;
        Ok(attributes)
    }
}

register_map! {
    #[description = "Independent address-space attribute registers. Hardware validates the complete combination on publication."]
    pub map ContextAttributeRegisters {
        alignment: 4,
        placements: [
            #[description = "Address-space mode encoded by ContextAttributes: Default is distinct from PASID zero."]
            pub address_space: Word,
            #[description = "Twenty-bit PASID when PASID mode is selected; must be zero in Default mode."]
            pub pasid: Word,
        ],
    }
}

register_map! {
    #[description = "Firmware-owned context binding. Publish a complete entry with valid; firmware clears valid only after successful Drain or Abort while keeping new submissions excluded. Live owner and attributes are immutable."]
    pub map ContextEntryRegisters {
        alignment: 4,
        placements: [
            #[description = "Function identity authorized to reference this context through its channels."]
            pub owner: Word,
            #[description = "Extensible address-space configuration resolved before memory access."]
            pub attributes: ContextAttributeRegisters,
            #[description = "Firmware publishes a fully configured entry by setting enabled. Firmware clears it only after successful cleanup with submissions still excluded; hardware never clears it as a maintenance side effect."]
            pub valid: Switch,
        ],
    }
}

register_schema! {
    #[short_description = "Atomic context maintenance request"]
    #[description = "One complete aligned eight-byte write submits the context selector and action together. Partial writes and requests while pending are rejected. Accepted requests self-clear; completion is reported by result."]
    pub register ContextRequest {
        width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0x0000_0003_ffff_ffff,
        fields: {
            #[description = "Context table index, not a PASID or Function identity. Ignored for Idle."]
            context_id: 0..=31,
            #[description = "Drain completes old work normally; Abort discards unstarted target commands. Neither invalidates the table entry. Idle does nothing; encoding three is invalid."]
            action: 32..=33 => enum { variants: { Idle = 0, Drain = 1, Abort = 2 } },
        },
    }
}

register_schema! {
    #[description = "Retained maintenance outcome. Pending and Failed never authorize release. Failed is published only after the failed-channel bitmap is complete; firmware owns subsequent recovery and valid clearing."]
    pub register ContextResult {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: {
            #[description = "Idle before the first request; Pending while draining; Success only at the cleanup boundary. InvalidContext rejects an absent entry; Busy rejects a conflicting operation or live ring binding; Failed requires recovery, with channel details in the tail bitmap."]
            state: 0..=2 => enum { variants: { Idle = 0, Pending = 1, Success = 2, InvalidContext = 3, Busy = 4, Failed = 5 } },
        },
    }
}

register_map! {
    #[description = "Firmware-only context management command and retained completion."]
    pub map ContextManagementRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Context table capacity"]
            #[description = "Read-only instance-configured number of context entries, not the number of free entries. Valid indices are zero through capacity minus one."]
            pub capacity: ReadWord,
            #[short_description = "Maintenance result"]
            #[description = "Read-only status of the latest accepted non-Idle request. Success establishes cleanup but leaves valid unchanged. On Failed, read the tail bitmap for all detected failing channels in the operation scope."]
            pub result: ContextResult,
            #[short_description = "Maintenance request"]
            #[description = "Firmware submits one context ID and Drain or Abort action in an atomic 64-bit transaction. Reads return zero after acceptance; use result to observe completion."]
            pub request: ContextRequest,
        ],
    }
}

register_map! {
    #[short_description = "Failed-channel bitmap word"]
    #[description = "Read-only failure snapshot, not a live fault mirror. Word n bit b identifies hardware ChannelId 32*n+b. Unimplemented channel bits remain zero."]
    pub map ChannelFailures {
        alignment: 4,
        placements: [
            #[description = "All detected failed channels in this word at the maintenance failure boundary. Frozen with the result until the next accepted non-Idle request; software cannot clear it."]
            pub bits: ReadWord,
        ],
    }
}

register_map! {
    #[description = "Instance-sized firmware-only context table and maintenance aperture. No context slot is reserved for control bootstrap."]
    pub map ContextTableRegisters(context_count: usize, bitmap_words: usize) {
        alignment: 8,
        placements: [
            #[description = "Firmware maintenance request, result and capacity discovery."]
            pub management: ContextManagementRegisters,
            #[description = "Firmware-allocated context bindings; owner and attributes are immutable while valid."]
            pub entries: ContextEntryRegisters[context_count],
            #[description = "Failure bitmap at the end of the aperture. Channel growth appends words without moving management or context entries; word count is ceil(channel_count/32)."]
            pub failed: ChannelFailures[bitmap_words],
        ],
    }
}

impl ContextTableRegisters {
    /// Derives the tail bitmap population from the hardware channel count.
    pub fn for_channels(
        context_count: usize,
        channel_count: usize,
    ) -> Result<ContextTableRegistersLayout, hardware_interface_schema::RegisterBankError> {
        Self::new(context_count, channel_count.div_ceil(32))
    }
}
