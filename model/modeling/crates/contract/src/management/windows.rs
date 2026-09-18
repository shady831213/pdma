use crate::{WindowControl, WindowStatus};
use hardware_interface_schema::register_map;

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
