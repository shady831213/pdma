use crate::register::{ReadWord, Switch, Word};
use hardware_interface_schema::register_map;

impl crate::dma::ChannelId {
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

register_map! {
    #[short_description = "Channel permission block"]
    #[description = "One channel's firmware permission control followed by its Host readiness status. Bank spacing derives from this complete block."]
    pub map ChannelWindowBlock {
        alignment: 4,
        placements: [
            pub control: WindowControl,
            pub status: WindowStatus,
        ],
    }
}

register_map! {
    #[short_description = "Channel permission windows"]
    #[description = "One complete permission block per hardware channel. Host and firmware see filtered views of this same physical bank."]
    pub map ChannelWindowRegisters(channel_count: usize) {
        alignment: 4,
        placements: [
            pub windows: ChannelWindowBlock[channel_count],
        ],
    }
}
