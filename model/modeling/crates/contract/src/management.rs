use hardware_interface_schema::register_map;

use crate::{ReadWord, Switch};

mod firmware;
pub use firmware::*;

/// Firmware-only access-window control address.
pub const WINDOW_CONTROL_BASE: u32 = 0xffff_ff00;
/// Host-visible access-window readiness address.
pub const WINDOW_STATUS_BASE: u32 = 0xffff_ff08;

register_map! {
    #[short_description = "Access-window management"]
    #[description = "Firmware-only management registers, reachable while downstream access is closed. Host accesses to this map are always denied."]
    pub map WindowControl {
        alignment: 4,
        placements: [
            #[short_description = "Downstream access enable"]
            #[description = "Bit zero opens downstream host access and publishes ready immediately. Zero closes access. Initially zero; other bits are reserved. Only firmware may read or write this control."]
            pub open: Switch,
        ],
    }
}

register_map! {
    #[short_description = "Access-window readiness"]
    #[description = "Host-readable status available while downstream access is closed. Hardware derives readiness directly from the management control without storing a separate handshake state."]
    pub map WindowStatus {
        alignment: 4,
        placements: [
            #[short_description = "Downstream access ready"]
            #[description = "One means downstream host access is open; zero means closed. Hardware derives the value from the firmware open control. Host writes are always denied."]
            pub ready: ReadWord,
        ],
    }
}
