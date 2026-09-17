use super::{ChannelBlock, DeviceControl};
use hardware_interface_schema::{checked_bank_offset, register_map, register_schema};

register_schema! {
    #[short_description = "Hardware channel count"]
    #[description = "Read-only count supplied by this hardware instance. Hardware initializes the value after construction and preserves it across device reset."]
    pub register ChannelCount {
        width: 4, endian: Little, access: ReadOnly,
        reset: PostReset(0), writable_mask: 0, fields: {},
    }
}

register_map! {
    #[short_description = "PDMA device registers"]
    #[description = "Device-wide discovery and recovery controls followed by the instance's independent channel blocks. Register transactions use little-endian bytes and must match an exact register start and width; layout gaps are not registers."]
    pub map DeviceRegisters(channel_count: usize) {
        alignment: 8,
        placements: [
            #[short_description = "Device management interface"]
            #[description = "Hardware channel discovery, device fatal notification and whole-device reset."]
            pub control: DeviceControl,
            #[short_description = "Independent channel interfaces"]
            #[description = "Repeated command/response rings and channel management. Each block owns its configuration, cursors, faults, notifications and recovery."]
            pub channels: ChannelBlock[channel_count],
        ],
    }
}

/// A channel selection, checked against hardware discovery by its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    /// Creates a channel selection without assuming a device's channel count.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based channel index.
    pub const fn index(self) -> usize {
        self.0
    }

    /// Returns the channel block address, or `None` on address arithmetic overflow.
    pub const fn register_offset(self) -> Option<usize> {
        checked_bank_offset(
            DeviceRegisters::CHANNELS_OFFSET,
            self.0,
            DeviceRegisters::CHANNELS_STRIDE,
        )
    }
}
