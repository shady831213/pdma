use hardware_interface_schema::{RegisterAccessError, RegisterBank, StaticRegisterLayout};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelWindowBlock, ChannelWindowRegisters, DeviceRegisters,
    WINDOW_BASE,
};

use crate::{AccessError, AccessWindow, WindowConfigError};

/// Independent firmware-controlled Host apertures for one configured DMA instance.
#[derive(Debug)]
pub struct ChannelWindows {
    windows: Vec<AccessWindow>,
    work: RegisterBank<StaticRegisterLayout<ChannelBlock>>,
    permissions: RegisterBank<StaticRegisterLayout<ChannelWindowBlock>>,
}

impl ChannelWindows {
    /// Creates closed channel windows, rejecting invalid or reserved-space geometry.
    pub fn new(channel_count: usize) -> Result<Self, WindowConfigError> {
        let layout = DeviceRegisters::new(channel_count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?;
        if layout.extent()
            > usize::try_from(WINDOW_BASE).map_err(|_| WindowConfigError::AddressUnrepresentable)?
        {
            return Err(WindowConfigError::ReservedAddressOverlap);
        }
        if ChannelId::new(channel_count - 1)
            .window_status_address()
            .is_none()
        {
            return Err(WindowConfigError::ReservedAddressOverlap);
        }
        let work = layout.channels().clone();
        let permissions = ChannelWindowRegisters::new(channel_count)
            .map_err(|_| WindowConfigError::InvalidChannelCount)?
            .windows()
            .rebased(
                usize::try_from(WINDOW_BASE)
                    .map_err(|_| WindowConfigError::AddressUnrepresentable)?,
            )
            .map_err(|_| WindowConfigError::AddressUnrepresentable)?;
        let windows = (0..channel_count)
            .map(|index| {
                let start = work
                    .offset(index)
                    .ok_or(WindowConfigError::AddressUnrepresentable)?;
                let base = permissions
                    .offset(index)
                    .ok_or(WindowConfigError::AddressUnrepresentable)?;
                Ok(AccessWindow::channel(
                    start..start + ChannelBlock::EXTENT,
                    base + ChannelWindowBlock::CONTROL.offset(),
                    base + ChannelWindowBlock::STATUS.offset(),
                ))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            windows,
            work,
            permissions,
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

    /// Reads readiness or forwards one wholly authorized Host transaction.
    pub fn read<E>(
        &self,
        address: usize,
        out: &mut [u8],
        downstream: impl FnOnce(usize, &mut [u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        match self.route(address, out.len())? {
            Some(window) => window.read(address, out, downstream),
            None => downstream(address, out).map_err(AccessError::Downstream),
        }
    }

    /// Forwards writes only within one independently granted aperture.
    pub fn write<E>(
        &self,
        address: usize,
        bytes: &[u8],
        downstream: impl FnOnce(usize, &[u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        match self.route(address, bytes.len())? {
            Some(window) => window.write(address, bytes, downstream),
            None => downstream(address, bytes).map_err(AccessError::Downstream),
        }
    }

    fn management_window(
        &mut self,
        address: usize,
    ) -> Result<&mut AccessWindow, RegisterAccessError> {
        let (index, _) = self
            .permissions
            .locate_instance(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        self.windows
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })
    }

    fn route<E>(
        &self,
        address: usize,
        width: usize,
    ) -> Result<Option<&AccessWindow>, AccessError<E>> {
        let end = address
            .checked_add(width)
            .filter(|_| width != 0)
            .ok_or(AccessError::Unmapped)?;
        if let Some((index, _)) = self.permissions.locate_instance(address) {
            return self
                .windows
                .get(index)
                .map(Some)
                .ok_or(AccessError::Unmapped);
        }
        if address < self.work.base() {
            if end > self.work.base() {
                return Err(AccessError::Unmapped);
            }
            return if self.windows.iter().any(AccessWindow::is_open) {
                Ok(None)
            } else {
                Err(AccessError::Closed)
            };
        }
        let (index, _) = self
            .work
            .locate_instance(address)
            .ok_or(AccessError::Unmapped)?;
        self.windows
            .get(index)
            .map(Some)
            .ok_or(AccessError::Unmapped)
    }
}
