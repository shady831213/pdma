use crate::{Channel, Dma};
use hardware_interface_schema::{RegisterBankError, RegisterWindow};
use pdma_model_contract::dma::{
    ChannelCountValues, DmaAccess, DmaAdmin, DmaControl, DmaDiscovery, DmaRegisters,
};
use std::{collections::TryReserveError, fmt};

/// Instance construction failure, separate from hardware or MMIO errors.
#[derive(Debug)]
pub enum BuildError {
    /// Channel count cannot be represented by the contract layout.
    Layout(RegisterBankError),
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

impl<M> Dma<M> {
    /// Creates an immutable channel population without allocating external DMA memory.
    pub fn new(memory: M, channel_count: usize) -> Result<Self, BuildError> {
        Self::with_sideband(memory, channel_count, |_| ())
    }
}

impl<M, S> Dma<M, S> {
    /// Tags each memory interaction with metadata derived from its channel.
    pub fn with_sideband(
        memory: M,
        channel_count: usize,
        sideband: fn(DmaAccess) -> S,
    ) -> Result<Self, BuildError> {
        let count = u32::try_from(channel_count).map_err(|_| BuildError::AddressSpace)?;
        let layout = DmaRegisters::new(channel_count).map_err(BuildError::Layout)?;
        let _ = u32::try_from(layout.extent()).map_err(|_| BuildError::AddressSpace)?;
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(channel_count)
            .map_err(BuildError::Allocation)?;
        channels.resize_with(channel_count, Channel::default);
        let control =
            RegisterWindow::<DmaControl>::new(layout.admin().base() + DmaAdmin::CONTROL.offset());
        let mut discovery = RegisterWindow::<DmaDiscovery>::new(DmaRegisters::DISCOVERY.offset());
        discovery.values.channel_num = ChannelCountValues::new(count);
        Ok(Self {
            layout,
            discovery,
            channels,
            memory,
            sideband,
            control,
            interrupt: false,
            active: None,
            maintenance: None,
        })
    }
}
