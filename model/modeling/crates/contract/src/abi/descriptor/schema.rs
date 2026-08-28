use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! {
    pub bits U32Word(u64) {
        value: 0..32,
    }
}

record_schema! {
    pub record AbiCommandHeader {
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
            size: u16 { const: AbiCommand::record_size },
            command_id: u64,
        },
    }
}

record_schema! {
    pub record AbiCommand {
        alignment: 64,
        endian: Little,
        fields: {
            header: AbiCommandHeader,
            payload: union {
                tag: self.header.opcode,
                cases: {
                    Copy {
                        source_dma_address: u64,
                        destination_dma_address: u64,
                        length: U32Word,
                    },
                    StridedCopy {
                        source_dma_address: u64,
                        destination_dma_address: u64,
                        rows: U32Word,
                        row_bytes: U32Word,
                        source_stride: U32Word,
                        destination_stride: U32Word,
                    },
                    BroadcastCopy {
                        source_dma_address: u64,
                        destination_list_dma_address: u64,
                        length: U32Word,
                        destination_count: U32Word,
                    },
                    VectorAdd {
                        source_a_dma_address: u64,
                        source_b_dma_address: u64,
                        destination_dma_address: u64,
                        length: U32Word,
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
