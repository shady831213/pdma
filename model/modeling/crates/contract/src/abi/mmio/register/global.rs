use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register GMagic { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Constant(0x414d_4450), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GMmioAbiVersion { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Constant(1), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GReqAbiVersion { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff, fields: { bits: 0..32, }, } }
register_schema! { pub register GReqFeatures { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register GResetExpectedEpoch { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register GCommand { width: 4, alignment: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register GResult { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GDetail0 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register GDetail1 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register GDetail2 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register GRspAbiVersion { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GRspRunState { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GRspFeatures { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register GRspEpoch { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: PostReset(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register GMaxRingDepth { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GMaxIndirectDestinations { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GMaxTransferBytes { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GMaxStridedRows { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register GMaxStride { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }

register_map! { pub map GlobalRegisters {
    alignment: 8,
    placements: [
        pub magic: GMagic,
        pub mmio_abi_version: GMmioAbiVersion,
        pub req_abi_version: GReqAbiVersion,
        pub req_features: GReqFeatures,
        pub reset_expected_epoch: GResetExpectedEpoch,
        pub command: GCommand,
        pub result: GResult,
        pub detail0: GDetail0,
        pub detail1: GDetail1,
        pub detail2: GDetail2,
        pub response: GlobalResponseRegisters,
    ],
} }

register_map! { pub map GlobalResponseRegisters {
    alignment: 8,
    placements: [
        pub abi_version: GRspAbiVersion,
        pub run_state: GRspRunState,
        pub features: GRspFeatures,
        pub epoch: GRspEpoch,
        pub max_ring_depth: GMaxRingDepth,
        pub max_indirect_destinations: GMaxIndirectDestinations,
        pub max_transfer_bytes: GMaxTransferBytes,
        pub max_strided_rows: GMaxStridedRows,
        pub max_stride: GMaxStride,
    ],
} }
