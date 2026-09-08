use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register GMagic { width: 4, endian: Little, access: ReadOnly, reset: Constant(0x414d_4450), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMmioVersion { width: 4, endian: Little, access: ReadOnly, reset: Constant(2), writable_mask: 0, fields: {}, } }
register_schema! { pub register GFeatures { width: 8, endian: Little, access: ReadOnly, reset: Constant(0xff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GEpoch { width: 8, endian: Little, access: ReadOnly, reset: PostReset(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register GRunState { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register GQueueCount { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMaxRingDepth { width: 4, endian: Little, access: ReadOnly, reset: Constant(0xffff_ffff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMaxIndirectDestinations { width: 4, endian: Little, access: ReadOnly, reset: Constant(0xffff_ffff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMaxTransferBytes { width: 4, endian: Little, access: ReadOnly, reset: Constant(0xffff_ffff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMaxStridedRows { width: 4, endian: Little, access: ReadOnly, reset: Constant(0xffff_ffff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GMaxStride { width: 4, endian: Little, access: ReadOnly, reset: Constant(0xffff_ffff), writable_mask: 0, fields: {}, } }
register_schema! { pub register GReset { width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 1, fields: {}, } }
register_schema! { pub register GAbiVersion { width: 4, endian: Little, access: ReadOnly, reset: Constant(1), writable_mask: 0, fields: {}, } }

register_map! { pub map GlobalRegisters {
    alignment: 8,
    placements: [
        pub magic: GMagic,
        pub mmio_version: GMmioVersion,
        pub features: GFeatures,
        pub epoch: GEpoch,
        pub run_state: GRunState,
        pub queue_count: GQueueCount,
        pub max_ring_depth: GMaxRingDepth,
        pub max_indirect_destinations: GMaxIndirectDestinations,
        pub max_transfer_bytes: GMaxTransferBytes,
        pub max_strided_rows: GMaxStridedRows,
        pub max_stride: GMaxStride,
        pub reset: GReset,
        pub abi_version: GAbiVersion,
    ],
} }
