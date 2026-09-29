use super::Reset;
use crate::register::{Address, ReadWord, Switch, Word};
use hardware_interface_schema::{register_map, register_schema};

register_map! {
    #[short_description = "Command ring registers"]
    #[description = "Software writes command records and publishes the producer cursor; hardware fetches records and advances the consumer cursor. Cursors are wrapping 32-bit entry counts, not byte offsets."]
    pub map CommandRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Command ring base address"]
            #[description = "Start of command storage in the attached memory backend, aligned to eight bytes. Command slots are 104 bytes apart. The value cannot change after the channel's first successful enable until reset."]
            pub base: Address,
            #[short_description = "Command ring capacity"]
            #[description = "Number of command slots. Must be a nonzero power of two below 2^31, with the complete ring extent representable in the address space. The value cannot change after the first successful enable."]
            pub capacity: Word,
            #[short_description = "Published command count"]
            #[description = "Software advances this wrapping cursor only after writing command records to memory. Publication may not exceed the ring's free capacity; hardware reads entries between consumer and producer."]
            pub producer: Word,
            #[short_description = "Fetched command count"]
            #[description = "Hardware advances this wrapping cursor after successfully reading and decoding a command. It releases the command slot; it does not indicate that the copy has completed."]
            pub consumer: ReadWord,
        ],
    }
}

register_map! {
    #[short_description = "Response ring registers"]
    #[description = "Hardware writes response records and publishes the producer cursor; software consumes records and advances the consumer cursor. A full ring stalls publication without replaying the copy."]
    pub map ResponseRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Response ring base address"]
            #[description = "Start of response storage in the attached memory backend, aligned to eight bytes. Response slots are 72 bytes apart. The value cannot change after the channel's first successful enable."]
            pub base: Address,
            #[short_description = "Response ring capacity"]
            #[description = "Number of response slots. Must be a nonzero power of two below 2^31, with the complete ring extent representable in the address space. The value cannot change after the first successful enable."]
            pub capacity: Word,
            #[short_description = "Published response count"]
            #[description = "Hardware advances this cursor only after writing a complete response. Empty-to-non-empty transitions and IOC publication independently request notification unless masked."]
            pub producer: ReadWord,
            #[short_description = "Consumed response count"]
            #[description = "Software advances this wrapping cursor after reading and decoding responses. It may consume only published entries. Draining the ring removes its interrupt condition, but does not clear a channel fault."]
            pub consumer: Word,
        ],
    }
}

register_schema! {
    #[short_description = "Channel fatal state"]
    #[description = "Hardware-owned sticky failure state. Software can observe the code; only channel or device reset clears it."]
    pub register ChannelFatal {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {
            #[short_description = "Fatal code"]
            #[description = "Distinguishes normal operation from command-fetch and response-publication failures. Other encodings are invalid."]
            code: 0..=31 => enum {
                variants: {
                    #[short_description = "No channel fatal"]
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
            #[description = "A stopping error response was published; processing is stopped until reset. PageFault does not set this cause."]
            response_error: 2..=2,
            #[description = "The channel fatal register contains a fatal error."]
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
    #[short_description = "Channel controls and fatal state"]
    #[description = "Controls channel processing and independent response notification sources. Stopping errors require reset; recoverable payload PageFault does not halt the channel. Ring-access fatal codes are distinct from command-level response status."]
    pub map ChannelRegisters {
        alignment: 4,
        placements: [
            #[short_description = "Channel processing enable"]
            #[description = "One permits pipeline progress and validates both ring geometries; the first successful enable locks their base addresses and capacities. Zero pauses processing without clearing cursors, retained work or errors."]
            pub enable: Switch,
            #[short_description = "Channel interrupt enable"]
            #[description = "One permits new channel notifications, including error and fatal. Zero suppresses them without clearing causes or retracting notifications already emitted. Enabling does not replay past events."]
            pub irq_enable: Switch,
            #[short_description = "Sticky channel fatal"]
            #[description = "A latched ring-access failure stops processing and contributes to IRQ. Only channel or device reset clears it."]
            pub fatal: ChannelFatal,
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
    #[description = "One independent channel's command/response rings, notification controls and sticky fatal. DmaRegisters repeats this data block for every implemented channel; recovery lives in ChannelAdmin."]
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
            #[description = "Processing enable, interrupt causes and sticky channel fatal. There is no separate event ring."]
            pub control: ChannelRegisters,
        ],
    }
}
