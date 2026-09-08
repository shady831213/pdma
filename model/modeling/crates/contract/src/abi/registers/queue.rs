use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub register QSqBase { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QSqDepth { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {}, } }
register_schema! { pub register QCqBase { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QCqDepth { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {}, } }
register_schema! { pub register QEqBase { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QEqDepth { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff, fields: {}, } }
register_schema! { pub register QEnable { width: 4, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 1, fields: {}, } }
register_schema! { pub register QSqProducer { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QCqConsumer { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QEqConsumer { width: 8, endian: Little, access: ReadWrite, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QDoorbell { width: 8, endian: Little, access: WriteOnly, reset: Zero(0), writable_mask: 0xffff_ffff_ffff_ffff, fields: {}, } }
register_schema! { pub register QSqCaptured { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register QSqConsumer { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register QCqProducer { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }
register_schema! { pub register QEqProducer { width: 8, endian: Little, access: ReadOnly, reset: Zero(0), writable_mask: 0, fields: {}, } }

register_map! { pub map QueueRegisters {
    alignment: 8,
    placements: [
        pub sq_base: QSqBase,
        pub sq_depth: QSqDepth,
        pub cq_base: QCqBase,
        pub cq_depth: QCqDepth,
        pub eq_base: QEqBase,
        pub eq_depth: QEqDepth,
        pub enable: QEnable,
        pub sq_producer: QSqProducer,
        pub cq_consumer: QCqConsumer,
        pub eq_consumer: QEqConsumer,
        pub doorbell: QDoorbell,
        pub sq_captured: QSqCaptured,
        pub sq_consumer: QSqConsumer,
        pub cq_producer: QCqProducer,
        pub eq_producer: QEqProducer,
    ],
} }
