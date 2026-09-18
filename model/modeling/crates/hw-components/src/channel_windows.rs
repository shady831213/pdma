use core::ops::Range;
use hardware_interface_schema::{
    RegisterAccessError, RegisterBank, RegisterShape, StaticRegisterLayout,
};
use pdma_model_contract::{
    ChannelBlock, ChannelWindowBlock, ChannelWindowRegisters, DmaDiscovery, DmaRegisters,
};

use crate::{AccessError, AccessWindow, WindowConfigError};

/// Independent firmware-controlled Host apertures for one configured DMA instance.
#[derive(Debug)]
pub struct ChannelWindows {
    windows: Vec<AccessWindow>,
    work: RegisterBank<StaticRegisterLayout<ChannelBlock>>,
    permissions: RegisterBank<StaticRegisterLayout<ChannelWindowBlock>>,
    discovery: Range<usize>,
    dma: Range<usize>,
}

impl ChannelWindows {
    /// Places closed data windows and their permission bank in platform address space.
    pub fn new(
        channel_count: usize,
        dma_base: u32,
        window_base: u32,
    ) -> Result<Self, WindowConfigError> {
        let layout =
            DmaRegisters::new(channel_count).map_err(|_| WindowConfigError::InvalidChannelCount)?;
        let dma_base =
            usize::try_from(dma_base).map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        let window_base =
            usize::try_from(window_base).map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        if dma_base % layout.alignment() != 0 {
            return Err(WindowConfigError::AddressUnrepresentable);
        }
        let dma_end = dma_base
            .checked_add(layout.extent())
            .ok_or(WindowConfigError::AddressUnrepresentable)?;
        let work = layout
            .channels()
            .rebased(dma_base)
            .map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        let permissions = ChannelWindowRegisters::new(channel_count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?
            .windows()
            .rebased(window_base)
            .map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        if dma_base < permissions.end() && permissions.base() < dma_end {
            return Err(WindowConfigError::ReservedAddressOverlap);
        }
        u32::try_from(dma_end - 1).map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        u32::try_from(permissions.end() - 1)
            .map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        let mut windows = Vec::new();
        windows
            .try_reserve_exact(channel_count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?;
        for index in 0..channel_count {
            let start = work
                .offset(index)
                .ok_or(WindowConfigError::AddressUnrepresentable)?;
            let base = permissions
                .offset(index)
                .ok_or(WindowConfigError::AddressUnrepresentable)?;
            windows.push(AccessWindow::new(
                start..start + ChannelBlock::EXTENT,
                base,
            )?);
        }
        let discovery = dma_base + DmaRegisters::DISCOVERY.offset();
        Ok(Self {
            windows,
            work,
            permissions,
            discovery: discovery..discovery + DmaDiscovery::EXTENT,
            dma: dma_base..dma_end,
        })
    }

    /// Reads one firmware control through its generated register decoder.
    pub fn management_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError> {
        self.management_window(address)?
            .management_read(address, out)
    }

    /// Changes only the addressed channel's permission control.
    pub fn management_write(
        &mut self,
        address: usize,
        bytes: &[u8],
    ) -> Result<(), RegisterAccessError> {
        self.management_window(address)?
            .management_write(address, bytes)
    }

    /// Reads public discovery, readiness or a granted data block.
    pub fn read<E>(
        &self,
        address: usize,
        out: &mut [u8],
        downstream: impl FnOnce(usize, &mut [u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        let end = address
            .checked_add(out.len())
            .filter(|_| !out.is_empty())
            .ok_or(AccessError::Unmapped)?;
        if self.discovery.contains(&address) && end <= self.discovery.end {
            return downstream(address, out).map_err(AccessError::Downstream);
        }
        self.route(address)?.read(address, out, downstream)
    }

    /// Grants writes only to the selected open data block.
    pub fn write<E>(
        &self,
        address: usize,
        bytes: &[u8],
        downstream: impl FnOnce(usize, &[u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        address
            .checked_add(bytes.len())
            .filter(|_| !bytes.is_empty())
            .ok_or(AccessError::Unmapped)?;
        self.route(address)?.write(address, bytes, downstream)
    }

    fn management_window(
        &mut self,
        address: usize,
    ) -> Result<&mut AccessWindow, RegisterAccessError> {
        let (index, _) = self
            .permissions
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.windows
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })
    }

    fn route<E>(&self, address: usize) -> Result<&AccessWindow, AccessError<E>> {
        if let Some((index, _)) = self.permissions.locate(address) {
            return self.windows.get(index).ok_or(AccessError::Unmapped);
        }
        if let Some((index, _)) = self.work.locate(address) {
            return self.windows.get(index).ok_or(AccessError::Unmapped);
        }
        if self.dma.contains(&address) {
            return Err(AccessError::Denied);
        }
        Err(AccessError::Unmapped)
    }
}
