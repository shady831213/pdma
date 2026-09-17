use hardware_interface_schema::register_map;

use crate::{ReadWord, Switch};

mod firmware;
pub use firmware::*;
mod windows;
pub use windows::*;

/// Base of the complete channel permission bank.
pub const WINDOW_BASE: u32 = 0xff00_0000;
/// Firmware-only access-window control address.
pub const WINDOW_CONTROL_BASE: u32 = WINDOW_BASE;
/// Host-visible access-window readiness address.
pub const WINDOW_STATUS_BASE: u32 = WINDOW_CONTROL_BASE + 8;
/// Byte distance between adjacent channel permission controls and statuses.
pub const WINDOW_STRIDE: usize = ChannelWindowRegisters::WINDOWS_STRIDE;

impl crate::ChannelId {
    /// Returns the firmware control address without entering always-on control space.
    pub fn window_control_address(self) -> Option<u32> {
        self.window_address(ChannelWindowBlock::CONTROL.offset())
    }

    fn window_address(self, offset: usize) -> Option<u32> {
        let base = hardware_interface_schema::checked_bank_offset(
            usize::try_from(WINDOW_BASE).ok()?,
            self.index(),
            WINDOW_STRIDE,
        )?;
        if base.checked_add(WINDOW_STRIDE)? > usize::try_from(crate::CONTROL_CHANNEL_BASE).ok()? {
            return None;
        }
        u32::try_from(base.checked_add(offset)?).ok()
    }

    /// Returns the Host readiness address for this channel permission window.
    pub fn window_status_address(self) -> Option<u32> {
        self.window_address(ChannelWindowBlock::STATUS.offset())
    }
}

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
