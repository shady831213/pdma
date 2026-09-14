use crate::{Channel, Device};
use pdma_model_contract::{
    ChannelCountValues, DeviceControlValues, DeviceLayoutError, DeviceRegisters,
};
use std::{collections::TryReserveError, fmt};

/// Instance construction failure, separate from hardware or MMIO errors.
#[derive(Debug)]
pub enum BuildError {
    /// Channel count cannot be represented by the contract layout.
    Layout(DeviceLayoutError),
    /// The register window exceeds the transport's 32-bit byte offsets.
    AddressSpace,
    /// Storage for channel state could not be allocated.
    Allocation(TryReserveError),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "PDMA construction: {self:?}")
    }
}
impl std::error::Error for BuildError {}

impl<M> Device<M> {
    /// Creates an immutable channel population without allocating external DMA memory.
    pub fn new(memory: M, channel_count: usize) -> Result<Self, BuildError> {
        let extent = DeviceRegisters::extent(channel_count).map_err(BuildError::Layout)?;
        let _ = u32::try_from(extent).map_err(|_| BuildError::AddressSpace)?;
        let count = u32::try_from(channel_count).map_err(|_| BuildError::AddressSpace)?;
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(channel_count)
            .map_err(BuildError::Allocation)?;
        channels.resize_with(channel_count, Channel::default);
        Ok(Self {
            channels,
            memory,
            control: DeviceControlValues {
                channel_num: ChannelCountValues::new(count),
                ..DeviceControlValues::default()
            },
            interrupt: false,
        })
    }
}
