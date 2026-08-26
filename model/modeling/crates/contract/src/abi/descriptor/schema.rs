use hardware_interface_schema::record_schema;

record_schema! {
    pub record DescriptorHeaderSchema {
        endian: Little,
        fields: {
            version: u16,
            opcode: enum(u16) {
                variants: {
                    Copy = 1,
                    StridedCopy,
                    BroadcastCopy,
                    VectorAdd,
                },
            },
            flags: bits(u16) {
                fields: {},
            },
            size: u16 { const: DescriptorSchema::record_size },
            command_id: u64,
        },
    }
}

record_schema! {
    pub record DescriptorSchema {
        alignment: 64,
        endian: Little,
        fields: {
            header: DescriptorHeaderSchema,
            payload: union {
                tag: self.header.opcode,
                cases: {
                    Copy {
                        source_dma_address: u64,
                        destination_dma_address: u64,
                        length: u32 { storage: u64 },
                    },
                    StridedCopy {
                        source_dma_address: u64,
                        destination_dma_address: u64,
                        rows: u32 { storage: u64 },
                        row_bytes: u32 { storage: u64 },
                        source_stride: u32 { storage: u64 },
                        destination_stride: u32 { storage: u64 },
                    },
                    BroadcastCopy {
                        source_dma_address: u64,
                        destination_list_dma_address: u64,
                        length: u32 { storage: u64 },
                        destination_count: u32 { storage: u64 },
                    },
                    VectorAdd {
                        source_a_dma_address: u64,
                        source_b_dma_address: u64,
                        destination_dma_address: u64,
                        length: u32 { storage: u64 },
                    },
                },
            },
        },
    }
}

record_schema! {
    pub record IndirectDestinationEntrySchema {
        alignment: 8,
        endian: Little,
        fields: {
            destination_dma_address: u64,
        }
    }
}
