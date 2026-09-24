use super::{CommandRegisters, DmaRegisters, Reset, ResponseRegisters};
use crate::register::{Switch, Word};
use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    #[short_description = "Channel fault state"]
    #[description = "Hardware-owned sticky failure state. Software can observe the code; only channel or device reset clears it."]
    pub register ChannelFault {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {
            #[short_description = "Fault code"]
            #[description = "Distinguishes normal operation from command-fetch and response-publication failures. Other encodings are invalid."]
            code: 0..=31 => enum {
                variants: {
                    #[short_description = "No channel fault"]
                    #[description = "Neither ring has latched a hardware access failure."]
                    None = 0,
                    #[short_description = "Command fetch failed"]
                    #[description = "Hardware could not fetch or decode a command ring entry; its consumer cursor was not advanced."]
                    CommandRead = 1,
                    #[short_description = "Response publication failed"]
                    #[description = "Hardware could not publish a response ring entry; its producer cursor was not advanced. The payload copy may already have completed."]
                    ResponseWrite = 2,
                },
            },
        },
    }
}

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

register_map! {
    #[short_description = "Channel administration"]
    #[description = "Privileged recovery aperture, separate from channel data registers."]
    pub map ChannelAdmin {
        alignment: 4,
        placements: [
            #[short_description = "Channel reset request and busy state"]
            #[description = "Requests channel cleanup; reads busy until accepted work retires. Other channels and DMA fatal state are unaffected."]
            pub reset: Reset,
        ],
    }
}

register_map! {
    #[short_description = "Channel controls and fault state"]
    #[description = "Controls channel processing and independent response notification sources. Any error stops the channel until reset. Ring-access fatal codes are distinct from command-level response status."]
    pub map ChannelRegisters {
        alignment: 4,
        placements: [
            #[short_description = "Channel processing enable"]
            #[description = "One permits pipeline progress and validates both ring geometries; the first successful enable locks their base addresses and capacities. Zero pauses processing without clearing cursors, retained work or faults."]
            pub enable: Switch,
            #[short_description = "Channel interrupt enable"]
            #[description = "One permits new channel notifications, including error and fatal. Zero suppresses them without clearing causes or retracting notifications already emitted. Enabling does not replay past events."]
            pub irq_enable: Switch,
            #[short_description = "Sticky channel fault"]
            #[description = "A latched ring-access failure stops processing and contributes to IRQ. Only channel or device reset clears it."]
            pub fault: ChannelFault,
            #[description = "Response non-empty, IOC, response error and channel fatal causes, visible even while masked."]
            pub irq_cause: ChannelIrqCause,
            #[description = "Independent non-empty and IOC notification masks. Unmask does not replay past events."]
            pub irq_mask: ChannelIrqMask,
            #[description = "Context shared by command and response ring accesses. Locked with ring geometry until reset completes."]
            pub ring_context: Word,
        ],
    }
}

register_map! {
    #[short_description = "PDMA channel register block"]
    #[description = "One independent channel's command/response rings, notification controls and sticky fault. DmaRegisters repeats this data block for every implemented channel; recovery lives in ChannelAdmin."]
    pub map ChannelBlock {
        alignment: 4096,
        extent: 4096,
        placements: [
            #[short_description = "Command queue interface"]
            #[description = "Registers owned by the software-produced, hardware-consumed command ring. Software programs storage before enabling the channel and publishes descriptors through its producer cursor."]
            pub command: CommandRegisters,
            #[short_description = "Response queue interface"]
            #[description = "Registers owned by the hardware-produced, software-consumed response ring. Software programs storage before enabling the channel and releases responses through its consumer cursor."]
            pub response: ResponseRegisters,
            #[short_description = "Channel management interface"]
            #[description = "Processing enable, interrupt causes and sticky channel fault. There is no separate event ring."]
            pub control: ChannelRegisters,
        ],
    }
}

/// A channel selection, checked against hardware discovery by its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    /// Creates a channel selection without assuming a device's channel count.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based channel index.
    pub const fn index(self) -> usize {
        self.0
    }

    /// Returns the data aperture for a valid population and selection, or `None` on overflow.
    pub fn register_offset(self, channel_count: usize) -> Option<usize> {
        DmaRegisters::new(channel_count)
            .ok()?
            .channels()
            .offset(self.0)
    }

    /// Returns the privileged recovery address for a valid population and selection.
    pub fn admin_offset(self, channel_count: usize) -> Option<usize> {
        let layout = DmaRegisters::new(channel_count).ok()?;
        layout
            .admin()
            .base()
            .checked_add(layout.admin().child().channels().offset(self.0)?)
    }
}
