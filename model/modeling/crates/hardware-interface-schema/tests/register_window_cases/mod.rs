mod access;
mod dispatch;
mod hooks;

use hardware_interface_schema::{register_map, register_schema};

register_schema! { register Control {
    width: 2, endian: Big, access: ReadWrite,
    reset: Constant(0xa103), writable_mask: 0xff0f, fields: {},
} }
register_schema! { register Status {
    width: 4, endian: Little, access: ReadOnly,
    reset: Constant(0x4433_2211), writable_mask: 0, fields: {},
} }
register_schema! { register Doorbell {
    width: 1, endian: Little, access: WriteOnly,
    reset: Zero(0), writable_mask: 0xff, fields: {},
} }
register_map! { map Map {
    alignment: 4, extent: 16,
    placements: [control: Control, status: Status { offset: 4 },
        doorbell: Doorbell { offset: 8 }],
} }

const BASE: usize = 0x1000;
