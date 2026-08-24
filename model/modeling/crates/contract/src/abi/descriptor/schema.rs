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
                        source_iova: u64,
                        destination_iova: u64,
                        length: u32 { storage: u64 },
                    },
                    StridedCopy {
                        source_iova: u64,
                        destination_iova: u64,
                        rows: u32 { storage: u64 },
                        row_bytes: u32 { storage: u64 },
                        source_stride: u32 { storage: u64 },
                        destination_stride: u32 { storage: u64 },
                    },
                    BroadcastCopy {
                        source_iova: u64,
                        destination_list_iova: u64,
                        length: u32 { storage: u64 },
                        destination_count: u32 { storage: u64 },
                    },
                    VectorAdd {
                        source_a_iova: u64,
                        source_b_iova: u64,
                        destination_iova: u64,
                        length: u32 { storage: u64 },
                    },
                },
            },
        },
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
