use hardware_interface_schema::register_schema;

use super::{U32Value, U64Value};

register_schema! { pub register IExpectedEpoch { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register ISourceId { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register ISourceKind { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRouteKind { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRouteVector { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register ICommand { width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IResult { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IDetail0 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IDetail1 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IDetail2 { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspEpoch { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspSourceId { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspSourceKind { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspState { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspRouteKind { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspNotificationKind { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U32Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspVector { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspMessageId { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
register_schema! { pub register IRspPendingMessages { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: U64Value { id: 0, lsb: 0 }, }, } }
