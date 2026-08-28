use hardware_interface_schema::record_schema;

record_schema! {
    pub record AbiCompletion {
        alignment: 32,
        endian: Little,
        fields: {
            version: u16,
            size: u16 { const: AbiCompletion::record_size },
            status: enum(u16) {
                variants: {
                    Success = 0,
                },
            },
            flags: bits(u16) {
                fields: {},
            },
            origin_queue_id: u64,
            command_id: u64,
            reserved: bits(u64) {
                fields: {},
            },
        }
    }
}

record_schema! {
    pub record AbiEvent {
        alignment: 64,
        endian: Little,
        fields: {
            version: u16,
            size: u16 { const: AbiEvent::record_size },
            kind: enum(u16) {
                variants: {
                    UnmappedDmaAddress = 1,
                    RangeOverflow,
                    OutOfBounds,
                    InvalidVersion,
                    InvalidSize,
                    InvalidOpcode,
                    InvalidFlags,
                    InvalidReserved,
                    InvalidIndirectCount,
                    InvalidIndirectAlignment,
                    ValueOutOfRange,
                },
            },
            flags: bits(u16) {
                fields: {},
            },
            origin_queue_id: u64,
            command_id: u64,
            fault_locator: u64,
            detail0: u64,
            detail1: u64,
            stage: enum(u16) {
                variants: {
                    Descriptor = 1,
                    IndirectList,
                    Source,
                    Destination,
                },
            },
            access: enum(u16) {
                variants: {
                    None = 0,
                    Read,
                    Write,
                },
            },
            reserved: bits(u32) {
                fields: {},
            },
            index: u64,
        }
    }
}
