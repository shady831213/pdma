//! Ordered register-layout macro behavior.

use hardware_interface_schema::{
    Decode, Encode, Endian, RegisterLayout, SchemaCodecError, SchemaMemberIdentity, register_map,
    register_schema,
};

include!("register_layout_dsl/located_child.rs");

#[path = "register_layout_dsl/errors.rs"]
mod errors;
#[path = "register_layout_dsl/layout.rs"]
mod layout;
#[path = "register_layout_dsl/values.rs"]
mod values;

register_schema! {
    pub register ControlRegister {
        width: 4,
        alignment: 4,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0x1020_3040),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

register_schema! {
    pub register CounterRegister {
        width: 8,
        alignment: 8,
        endian: Little,
        access: ReadOnly,
        reset: PostReset(0x0102_0304_0506_0708),
        writable_mask: 0,
        fields: {},
    }
}

register_map! {
    pub map LaneRegisters {
        alignment: 8,
        placements: [
            control: ControlRegister,
            counter: CounterRegister,
        ],
    }
}

register_map! {
    pub map DeviceRegisters {
        alignment: 16,
        extent: 64,
        placements: [
            header: ControlRegister,
            lanes: LaneRegisters { count: 2 },
            tail: ControlRegister { offset: 56 },
        ],
    }
}

register_map! {
    pub map LocatedChildMap {
        alignment: 4,
        placements: [
            children: LocatedChild { count: 2 },
        ],
    }
}

register_map! {
    pub map NestedLocatedChildMap {
        alignment: 4,
        placements: [
            maps: LocatedChildMap { count: 2 },
        ],
    }
}
