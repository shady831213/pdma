use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register QExpectedEpoch { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QId { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QExpectedCursor { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QNewCursor { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QCfgSqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QCfgCqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QCfgEventPolicy { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register QCfgSoftwareOwner { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QSqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QSqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QCqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QCqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QEqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register QEqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QCommand { width: 4, alignment: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register QResult { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register QDetail0 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QDetail1 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QDetail2 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspEpoch { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspId { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspSqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspSqCaptured { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspSqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspCqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspCqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspEqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register QRspEqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }

register_map! { pub map QueueRegisters {
    alignment: 8,
    placements: [
        pub expected_epoch: QExpectedEpoch,
        pub id: QId,
        pub expected_cursor: QExpectedCursor,
        pub new_cursor: QNewCursor,
        pub cfg_sq_depth: QCfgSqDepth,
        pub cfg_cq_depth: QCfgCqDepth,
        pub cfg_event_policy: QCfgEventPolicy,
        pub cfg_software_owner: QCfgSoftwareOwner,
        pub sq_base: QSqBase,
        pub sq_depth: QSqDepth,
        pub cq_base: QCqBase,
        pub cq_depth: QCqDepth,
        pub eq_base: QEqBase,
        pub eq_depth: QEqDepth,
        pub command: QCommand,
        pub result: QResult,
        pub detail0: QDetail0,
        pub detail1: QDetail1,
        pub detail2: QDetail2,
        pub response: QueueResponseRegisters,
    ],
} }

register_map! { pub map QueueResponseRegisters {
    alignment: 8,
    placements: [
        pub epoch: QRspEpoch,
        pub queue_id: QRspId,
        pub sq_producer: QRspSqProducer,
        pub sq_captured: QRspSqCaptured,
        pub sq_consumer: QRspSqConsumer,
        pub cq_producer: QRspCqProducer,
        pub cq_consumer: QRspCqConsumer,
        pub eq_producer: QRspEqProducer,
        pub eq_consumer: QRspEqConsumer,
    ],
} }
