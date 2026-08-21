use hardware_interface_schema::{register_map, register_schema};

use super::{U32Value, U64Value};

register_schema! { pub register QExpectedEpoch { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QId { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QExpectedCursor { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QNewCursor { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCfgSqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCfgCqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCfgEventPolicy { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCfgSoftwareOwner { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QSqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QSqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QEqBase { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QEqDepth { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QCommand { width: 4, alignment: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QResult { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QDetail0 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QDetail1 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QDetail2 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspEpoch { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspId { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspSqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspSqCaptured { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspSqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspCqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspCqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspEqProducer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register QRspEqConsumer { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }

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
