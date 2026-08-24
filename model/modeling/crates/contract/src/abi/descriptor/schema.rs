use hardware_interface_schema::record_schema;

record_schema! {
    pub record DescriptorSchema {
        size: 64,
        alignment: 64,
        fields: {
            version: u16 { id: 1, offset: 0, endian: Little },
            opcode: u16 { id: 2, offset: 2, endian: Little },
            flags: u16 { id: 3, offset: 4, endian: Little },
            size: u16 { id: 4, offset: 6, endian: Little },
            command_id: u64 { id: 5, offset: 8, endian: Little },
            word0: u64 { id: 6, offset: 16, endian: Little },
            word1: u64 { id: 7, offset: 24, endian: Little },
            word2: u64 { id: 8, offset: 32, endian: Little },
            word3: u64 { id: 9, offset: 40, endian: Little },
            word4: u64 { id: 10, offset: 48, endian: Little },
            word5: u64 { id: 11, offset: 56, endian: Little },
        }
    }
}

record_schema! {
    pub record IndirectDestinationEntrySchema {
        size: 8,
        alignment: 8,
        fields: {
            destination_iova: u64 { id: 1, offset: 0, endian: Little },
        }
    }
}
