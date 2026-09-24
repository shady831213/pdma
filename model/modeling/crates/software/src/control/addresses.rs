use pdma_model_contract::dma::ChannelId;

/// Platform-supplied register bases in the software program's bus address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManagementAddresses {
    /// DMA register aperture.
    pub dma: u32,
    /// Firmware-managed channel permission bank.
    pub windows: u32,
    /// Control queue bootstrap and doorbells.
    pub control: u32,
    /// Firmware boot report.
    pub firmware: u32,
    /// Optional firmware context-table aperture.
    pub contexts: Option<u32>,
}

impl ManagementAddresses {
    /// Resolves a channel's privileged permission control.
    pub fn window_control(self, channel: ChannelId) -> Option<u32> {
        self.windows
            .checked_add(u32::try_from(channel.window_control_offset()?).ok()?)
    }

    /// Resolves a channel's Host-readable permission status.
    pub fn window_status(self, channel: ChannelId) -> Option<u32> {
        self.windows
            .checked_add(u32::try_from(channel.window_status_offset()?).ok()?)
    }
}
