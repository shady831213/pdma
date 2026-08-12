use hardware_interface_schema::register_schema;

use super::{U32Value, U64Value};

register_schema! { pub register GMagic { width: 4, endian: Little, access: ReadOnly, reset: Constant(0x414d_4450), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMmioAbiVersion { width: 4, endian: Little, access: ReadOnly, reset: Constant(1), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GReqAbiVersion { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GReqFeatures { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GResetExpectedEpoch { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GCommand { width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GResult { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GDetail0 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GDetail1 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GDetail2 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GRspAbiVersion { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GRspRunState { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GRspFeatures { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GRspEpoch { width: 8, endian: Little, access: ReadOnly, reset: PostReset(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMaxRingDepth { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMaxIndirectDestinations { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMaxTransferBytes { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMaxStridedRows { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register GMaxStride { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
