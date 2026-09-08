use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register IExpectedEpoch { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register ISourceId { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register ISourceKind { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: { bits: 0..32, }, } }
register_schema! { pub register IRouteKind { width: 4, alignment: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: { bits: 0..32, }, } }
register_schema! { pub register IRouteVector { width: 8, alignment: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: { bits: 0..64, }, } }
register_schema! { pub register ICommand { width: 4, alignment: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff, fields: { bits: 0..32, }, } }
register_schema! { pub register IResult { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register IDetail0 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IDetail1 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IDetail2 { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IRspEpoch { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IRspSourceId { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IRspSourceKind { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register IRspState { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register IRspRouteKind { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register IRspNotificationKind { width: 4, alignment: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..32, }, } }
register_schema! { pub register IRspVector { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IRspMessageId { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }
register_schema! { pub register IRspPendingMessages { width: 8, alignment: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: { bits: 0..64, }, } }

register_map! { pub map IrqRegisters {
    alignment: 8,
    placements: [
        pub expected_epoch: IExpectedEpoch,
        pub source_id: ISourceId,
        pub source_kind: ISourceKind,
        pub route_kind: IRouteKind,
        pub route_vector: IRouteVector,
        pub command: ICommand,
        pub result: IResult,
        pub detail0: IDetail0,
        pub detail1: IDetail1,
        pub detail2: IDetail2,
        pub response: IrqResponseRegisters,
    ],
} }

register_map! { pub map IrqResponseRegisters {
    alignment: 8,
    placements: [
        pub epoch: IRspEpoch,
        pub source_id: IRspSourceId,
        pub source_kind: IRspSourceKind,
        pub state: IRspState,
        pub route_kind: IRspRouteKind,
        pub notification_kind: IRspNotificationKind,
        pub vector: IRspVector,
        pub message_id: IRspMessageId,
        pub pending_messages: IRspPendingMessages,
    ],
} }
