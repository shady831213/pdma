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
            #[description = "Start of command storage in the attached memory backend, aligned to eight bytes. Command slots are 112 bytes apart. The value cannot change after the channel's first successful enable until reset."]
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
    #[short_description = "Event ring registers"]
    #[description = "Hardware writes event records and publishes the producer cursor; software consumes records and advances the consumer cursor. A full ring is fatal only when an error event must be published; successful work needs no event capacity."]
    pub map EventRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Event ring base address"]
            #[description = "Start of event storage in the attached memory backend, aligned to eight bytes. Event slots are 24 bytes apart. The value cannot change after the channel's first successful enable."]
            pub base: Address,
            #[short_description = "Event ring capacity"]
            #[description = "Number of event slots. Must be a nonzero power of two below 2^31, with the complete ring extent representable in the address space. The value cannot change after the first successful enable."]
            pub capacity: Word,
            #[short_description = "Published event count"]
            #[description = "Hardware advances this cursor only after writing a complete event. Publication requests an error notification and optionally an empty-to-non-empty notification; Trap never publishes an event."]
            pub producer: ReadWord,
            #[short_description = "Consumed event count"]
            #[description = "Software advances this wrapping cursor after reading and decoding events. It may consume only published entries. Draining the ring removes its interrupt condition, but does not clear a channel fault."]
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
            #[description = "Distinguishes normal operation from command-fetch and event-publication failures. Other encodings are invalid."]
            code: 0..=31 => enum {
                variants: {
                    #[short_description = "No channel fatal"]
                    #[description = "Neither ring has latched a hardware access failure."]
                    None = 0,
                    #[short_description = "Command fetch failed"]
                    #[description = "Hardware could not fetch or decode a command ring entry; its consumer cursor was not advanced."]
                    CommandRead = 1,
                    #[short_description = "Event publication failed"]
                    #[description = "Hardware could not publish an event ring entry; its producer cursor was not advanced. The payload copy may already have completed."]
                    EventWrite = 2,
                    #[short_description = "Error publication encountered a full event ring"]
                    #[description = "Error publication encountered a full event ring."]
                    EventFull = 3,
                    #[short_description = "Error publication encountered invalid event geometry or cursors"]
                    #[description = "Error publication encountered invalid event geometry or cursors."]
                    EventConfiguration = 4,
                },
            },
        },
    }
}

register_schema! {
    #[short_description = "Channel interrupt causes"]
    #[description = "Hardware causes visible regardless of masks. Reading does not acknowledge notifications. Non-empty follows occupancy, Trap clears on write-one acknowledgment, and error/fatal require channel reset."]
    pub register ChannelIrqCause {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0,
        fields: {
            #[short_description = "The event ring currently contains unread entries"]
            #[description = "The event ring currently contains unread entries."]
            event_non_empty: 0..=0,
            #[short_description = "A Trap command executed since the last Trap acknowledgment"]
            #[description = "A Trap command executed since the last Trap acknowledgment."]
            trap: 1..=1,
            #[short_description = "A stopping command error is latched before event publication; processing remains stopped until reset"]
            #[description = "A stopping command error is latched before event publication; processing remains stopped until reset. Payload suspension does not set this cause."]
            error: 2..=2,
            #[short_description = "The channel fatal register contains a fatal error"]
            #[description = "The channel fatal register contains a fatal error."]
            channel_fatal: 3..=3,
        },
    }
}

register_schema! {
    #[short_description = "Channel event notification masks"]
    #[description = "One suppresses that source's future notifications without clearing causes. Unmask does not replay past events; software must check queue and error state before waiting. Error and fatal notifications bypass these two masks."]
    pub register ChannelIrqMask {
        width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 3,
        fields: {
            #[short_description = "Mask notifications on event empty-to-non-empty transitions"]
            #[description = "Mask notifications on event empty-to-non-empty transitions."]
            event_non_empty: 0..=0,
            #[short_description = "Mask notifications from Trap commands; each unmasked Trap may emit even with sticky cause already set"]
            #[description = "Mask notifications from Trap commands; each unmasked Trap may emit even with sticky cause already set."]
            trap: 1..=1,
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
    #[short_description = "Channel configuration"]
    #[description = "Processing enable remains writable; ring context freezes after the first successful enable."]
    pub map ChannelConfiguration {
        alignment: 4,
        placements: [
            #[short_description = "Channel processing enable"]
            #[description = "One permits pipeline progress and validates both ring geometries; the first successful enable locks their base addresses and capacities. Zero pauses processing without clearing cursors, retained work or errors."]
            pub enable: Switch,
            #[short_description = "Context shared by command and event ring accesses"]
            #[description = "Context shared by command and event ring accesses. Locked with ring geometry until reset completes."]
            pub ring_context: Word,
        ],
    }
}

register_map! {
    #[short_description = "Channel failure and resume"]
    #[description = "Channel-owned fatal status and recoverable payload suspension. Privileged reset remains in ChannelAdmin."]
    pub map ChannelRecovery {
        alignment: 4,
        placements: [
            #[short_description = "Sticky channel fatal"]
            #[description = "A latched ring-access failure stops processing and contributes to IRQ. Only channel or device reset clears it."]
            pub fatal: ChannelFatal,
            #[short_description = "Payload fault status"]
            #[description = "The faulted bit is set while a retained command awaits software repair and resume. Fault details are reported by the platform, without a DMA event or interrupt."]
            pub fault_status: FaultStatus,
            #[short_description = "Resume suspended command"]
            #[description = "Write one after repairing mappings to clear faulted and permit retry of retained work. Does not enable processing, clear errors or complete the command. Software serializes fault handling and resume with reset, release and reassignment."]
            pub resume: Resume,
        ],
    }
}

register_map! {
    #[short_description = "Channel interrupt controls"]
    #[description = "Independent channel notification enable, causes, masks and Trap acknowledgment."]
    pub map ChannelInterrupts {
        alignment: 4,
        placements: [
            #[short_description = "Channel interrupt enable"]
            #[description = "One permits new channel notifications, including error and fatal. Zero suppresses them without clearing causes or retracting notifications already emitted. Enabling does not replay past events."]
            pub enable: Switch,
            #[short_description = "Event non-empty, Trap, event error and channel fatal causes, visible even while masked"]
            #[description = "Event non-empty, Trap, event error and channel fatal causes, visible even while masked."]
            pub cause: ChannelIrqCause,
            #[short_description = "Independent non-empty and Trap notification masks"]
            #[description = "Independent non-empty and Trap notification masks. Unmask does not replay past events."]
            pub mask: ChannelIrqMask,
            #[short_description = "Write one to clear sticky Trap cause without retracting platform notifications"]
            #[description = "Write one to clear sticky Trap cause without retracting platform notifications."]
            pub trap_ack: TrapAck,
        ],
    }
}

register_map! {
    #[short_description = "Channel controls and fatal state"]
    #[description = "Channel-owned configuration, recovery and interrupt groups. Stopping errors require reset; recoverable payload faults retain the command until explicit resume."]
    pub map ChannelRegisters {
        alignment: 4,
        placements: [
            #[description = "Processing enable and locked ring context."]
            pub config: ChannelConfiguration,
            #[description = "Sticky fatal status and payload fault recovery."]
            pub recovery: ChannelRecovery,
            #[description = "Notification enable, causes, masks and Trap acknowledgment."]
            pub irq: ChannelInterrupts,
        ],
    }
}

register_map! {
    #[short_description = "PDMA channel register block"]
    #[description = "One independent channel's command/event rings, notification controls and sticky fatal. DmaRegisters repeats this data block for every implemented channel; privileged reset lives in ChannelAdmin."]
    pub map ChannelBlock {
        alignment: 4096,
        extent: 4096,
        placements: [
            #[short_description = "Command queue interface"]
            #[description = "Registers owned by the software-produced, hardware-consumed command ring. Software programs storage before enabling the channel and publishes descriptors through its producer cursor."]
            pub command: CommandRegisters,
            #[short_description = "Event queue interface"]
            #[description = "Registers owned by the hardware-produced, software-consumed event ring. Software programs storage before enabling the channel and releases events through its consumer cursor."]
            pub event: EventRegisters,
            #[short_description = "Channel management interface"]
            #[description = "Processing enable, interrupt causes and sticky channel fatal. Trap notifications do not use event storage."]
            pub control: ChannelRegisters,
        ],
    }
}

register_schema! {
    #[short_description = "Payload fault status"]
    #[description = "Hardware sets faulted when a payload access returns a recoverable page fault. Resume clears it; a later fault sets it again. Zero is not command completion. Reset or matching Abort discards parked work and clears the bit."]
    pub register FaultStatus {
        width: 4, endian: Little, access: ReadOnly, reset: Zero(0),
        writable_mask: 0, fields: {
            #[short_description = "A payload page fault awaits software repair and an explicit resume request"]
            #[description = "A payload page fault awaits software repair and an explicit resume request."]
            faulted: 0..=0,
        },
    }
}

register_schema! {
    #[short_description = "Payload resume request"]
    #[description = "Write-one action for the current channel fault. Zero writes have no effect; requests made while not faulted are discarded, not saved for later faults. No request identity or duplicate suppression is provided."]
    pub register Resume {
        width: 4, endian: Little, access: WriteOnly, reset: Zero(0),
        writable_mask: 1, fields: {
            #[short_description = "Write one to clear faulted after mapping repair; reserved bits must be zero"]
            #[description = "Write one to clear faulted after mapping repair; reserved bits must be zero."]
            request: 0..=0,
        },
    }
}

register_schema! {
    #[short_description = "Write-one acknowledgment of sticky Trap cause; zero is ignored and pending platform notifications survive"]
    #[description = "Write-one acknowledgment of sticky Trap cause; zero is ignored and pending platform notifications survive."]
    pub register TrapAck {
        width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 1,
        fields: {
            #[short_description = "Write one to clear the sticky Trap cause"]
            #[description = "Write one to clear the sticky Trap cause."]
            request: 0..=0,
        },
    }
}
