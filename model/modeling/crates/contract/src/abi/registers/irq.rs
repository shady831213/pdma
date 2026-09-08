use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register IRouteKind { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: {}, } }
register_schema! { pub register IRouteVector { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register IEnabled { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: {}, } }
register_schema! { pub register IMasked { width: 4, endian: Little, access: ReadWrite, reset: Constant(1), writable_mask: 1, fields: {}, } }
register_schema! { pub register IAck { width: 4, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 1, fields: {}, } }
register_schema! { pub register IStatus { width: 4, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register IPendingMessages { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }

register_map! { pub map IrqSourceRegisters {
    alignment: 8,
    placements: [
        pub route_kind: IRouteKind,
        pub route_vector: IRouteVector,
        pub enabled: IEnabled,
        pub masked: IMasked,
        pub ack: IAck,
        pub status: IStatus,
        pub pending_messages: IPendingMessages,
    ],
} }
