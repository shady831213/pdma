use crate::register::{Address, ReadWord, Word};
use hardware_interface_schema::register_map;

register_map! {
    #[short_description = "Command ring registers"]
    #[description = "Software writes command records and publishes the producer cursor; hardware fetches records and advances the consumer cursor. Cursors are wrapping 32-bit entry counts, not byte offsets."]
    pub map CommandRegisters {
        alignment: 8,
        placements: [
            #[short_description = "Command ring base address"]
            #[description = "Start of command storage in the attached memory backend, aligned to eight bytes. Command slots are 64 bytes apart. The value cannot change after the channel's first successful enable until reset."]
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
            #[description = "Hardware advances this cursor only after writing a complete response. Empty-to-non-empty transitions and IOC publication independently request notification unless masked."]
            pub producer: ReadWord,
            #[short_description = "Consumed response count"]
            #[description = "Software advances this wrapping cursor after reading and decoding responses. It may consume only published entries. Draining the ring removes its interrupt condition, but does not clear a channel fault."]
            pub consumer: Word,
        ],
    }
}
