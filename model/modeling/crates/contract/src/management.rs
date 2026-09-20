use hardware_interface_schema::register_map;

use crate::{ReadWord, Switch, Word};

/// Platform-assigned identity, also used as the explicit target of trusted PF requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Constructs an identity in the platform's Function namespace.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the register encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

mod firmware;
pub use firmware::*;
mod windows;
pub use windows::*;

impl crate::ChannelId {
    /// Returns this channel's control offset within the permission bank.
    pub const fn window_control_offset(self) -> Option<usize> {
        self.window_offset(ChannelWindowBlock::CONTROL.offset())
    }

    const fn window_offset(self, offset: usize) -> Option<usize> {
        match hardware_interface_schema::checked_bank_offset(
            ChannelWindowRegisters::WINDOWS_OFFSET,
            self.index(),
            ChannelWindowRegisters::WINDOWS_STRIDE,
        ) {
            Some(base) => base.checked_add(offset),
            None => None,
        }
    }

    /// Returns this channel's readiness offset within the permission bank.
    pub const fn window_status_offset(self) -> Option<usize> {
        self.window_offset(ChannelWindowBlock::STATUS.offset())
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
            #[description = "Function authorized to access this channel and receive its notifications. Firmware may change this binding only while the window is closed, after draining old channel work. Zero is the ordinary single-Host identity."]
            pub owner: Word,
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
