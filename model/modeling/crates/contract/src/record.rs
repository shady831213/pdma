use hardware_interface_schema::record_schema;

record_schema! {
    pub record Command {
        alignment: 8,
        endian: Little,
        fields: {
            id: u64,
            source: u64,
            destination: u64,
            length: u32,
        },
    }
}

record_schema! {
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            id: u64,
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadFault = 2,
                    WriteFault = 3,
                },
            },
        },
    }
}
