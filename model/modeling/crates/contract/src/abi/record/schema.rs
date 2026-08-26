use hardware_interface_schema::record_schema;

record_schema! {
    pub record CompletionSchema {
        alignment: 32,
        endian: Little,
        fields: {
            version: u16,
            size: u16,
            status: u16,
            flags: u16,
            origin_queue_id: u64,
            command_id: u64,
            reserved: u64,
        }
    }
}

record_schema! {
    pub record EventSchema {
        alignment: 64,
        endian: Little,
        fields: {
            version: u16,
            size: u16,
            kind: u16,
            flags: u16,
            origin_queue_id: u64,
            command_id: u64,
            fault_locator: u64,
            detail0: u64,
            detail1: u64,
            stage: u16,
            access: u16,
            reserved: u32,
            index: u64,
        }
    }
}
