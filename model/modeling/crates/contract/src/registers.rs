use hardware_interface_schema::{register_map, register_schema};

register_schema! {
    #[short_description = "Read/write word"]
    #[description = "A 32-bit little-endian register, initially zero. Software may read or write all bits; its placement defines their hardware meaning."]
    pub register Word {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Read-only word"]
    #[description = "A 32-bit little-endian register, initially zero. Hardware updates the value; software may read it but cannot write it."]
    pub register ReadWord {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Channel fault state"]
    #[description = "Hardware-owned sticky failure state. Software can observe the code but cannot clear it."]
    pub register ChannelFault {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {
            #[short_description = "Fault code"]
            #[description = "Distinguishes normal operation from command-fetch and response-publication failures. Other encodings are invalid."]
            code: 0..32 => enum {
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
    #[short_description = "Memory address"]
    #[description = "A 64-bit little-endian address in the device's attached memory backend. The backend determines address interpretation; the value need not be a host pointer."]
    pub register Address {
        width: 8,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff_ffff_ffff,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Enable control"]
    #[description = "A 32-bit little-endian control register, initially disabled. Only bit zero is writable; bits 1 through 31 are reserved and remain zero."]
    pub register Switch {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 1,
        fields: {
            #[short_description = "Enabled"]
            #[description = "Zero disables the function and one enables it. The containing register placement specifies the controlled function and its side effects."]
            enabled: 0..1,
        },
    }
}

register_map! {
    #[short_description = "Command ring registers"]
    #[description = "Software writes command records and publishes the producer cursor; hardware fetches records and advances the consumer cursor. Cursors are wrapping 32-bit entry counts, not byte offsets."]
    pub map CommandRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Command ring base address"]
            #[description = "Start of command storage in the attached memory backend, aligned to eight bytes. Command slots are 56 bytes apart. The value cannot change after the channel's first successful enable."]
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
            #[description = "Start of response storage in the attached memory backend, aligned to eight bytes. Response slots are 24 bytes apart. The value cannot change after the channel's first successful enable."]
            pub base: Address,
            #[short_description = "Response ring capacity"]
            #[description = "Number of response slots. Must be a nonzero power of two below 2^31, with the complete ring extent representable in the address space. The value cannot change after the first successful enable."]
            pub capacity: Word,
            #[short_description = "Published response count"]
            #[description = "Hardware advances this wrapping cursor only after successfully writing a complete response to memory. A difference from consumer represents unread responses and contributes to the level interrupt."]
            pub producer: ReadWord,
            #[short_description = "Consumed response count"]
            #[description = "Software advances this wrapping cursor after reading and decoding responses. It may consume only published entries. Draining the ring removes its interrupt condition, but does not clear a channel fault."]
            pub consumer: Word,
        ],
    }
}

register_map! {
    #[short_description = "Channel controls and fault state"]
    #[description = "Controls processing and the level interrupt for the command/response ring pair. Ring-access faults are sticky and stop processing; they are distinct from command-level response status."]
    pub map ChannelRegisters {
        alignment: 4,
        placements: [
            #[short_description = "Channel processing enable"]
            #[description = "One permits pipeline progress and validates both ring geometries; the first successful enable locks their base addresses and capacities. Zero pauses processing without clearing cursors, retained work or faults."]
            pub enable: Switch,
            #[short_description = "Channel interrupt enable"]
            #[description = "One asserts the level interrupt while responses are unread or a channel fault is latched. Zero masks the line without clearing either condition. Writing this control immediately refreshes the line."]
            pub irq_enable: Switch,
            #[short_description = "Sticky channel fault"]
            #[description = "A latched ring-access failure stops processing and contributes to IRQ. Software cannot clear it; this minimal model requires a new channel for recovery."]
            pub fault: ChannelFault,
        ],
    }
}

register_map! {
    #[short_description = "Minimal PDMA device registers"]
    #[description = "One channel with a command ring, a response ring and a level interrupt. Register transactions use little-endian bytes and must match an exact register start and width; layout gaps are not registers."]
    pub map DeviceRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Command queue interface"]
            #[description = "Registers owned by the software-produced, hardware-consumed command ring. Software programs storage before enabling the channel and publishes descriptors through its producer cursor."]
            pub command: CommandRegisters,
            #[short_description = "Response queue interface"]
            #[description = "Registers owned by the hardware-produced, software-consumed response ring. Software programs storage before enabling the channel and releases responses through its consumer cursor."]
            pub response: ResponseRegisters,
            #[short_description = "Channel management interface"]
            #[description = "Processing enable, interrupt enable and sticky ring-access fault state shared by the two rings. There is no separate event ring or software fault-clear register in this model."]
            pub channel: ChannelRegisters,
        ],
    }
}
