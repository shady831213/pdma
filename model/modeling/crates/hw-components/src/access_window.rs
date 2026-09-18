use hardware_interface_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::{ChannelWindowBlock, ReadWordValues, WindowControl, WindowStatus};

use crate::{AccessError, WindowConfigError};

/// Firmware-controlled permission gate for a concrete downstream register range.
#[derive(Debug)]
pub struct AccessWindow {
    control: RegisterWindow<WindowControl>,
    extent: usize,
    start: usize,
    status_base: usize,
}

impl AccessWindow {
    /// Creates one closed aperture and its separately placed permission block.
    pub fn new(range: core::ops::Range<usize>, base: usize) -> Result<Self, WindowConfigError> {
        let end = base
            .checked_add(ChannelWindowBlock::EXTENT)
            .ok_or(WindowConfigError::AddressUnrepresentable)?;
        if range.start > range.end || base % ChannelWindowBlock::ALIGNMENT != 0 {
            return Err(WindowConfigError::AddressUnrepresentable);
        }
        if range.start < end && base < range.end {
            return Err(WindowConfigError::ReservedAddressOverlap);
        }
        Ok(Self {
            control: RegisterWindow::new(base + ChannelWindowBlock::CONTROL.offset()),
            start: range.start,
            extent: range.end,
            status_base: base + ChannelWindowBlock::STATUS.offset(),
        })
    }

    /// Reads the firmware-only control map even while downstream access is closed.
    pub fn management_read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError> {
        self.control.read(address, out)
    }

    /// Writes the firmware-only control map; opening publishes readiness immediately.
    pub fn management_write(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError> {
        self.control.write(address, source)
    }

    /// Returns the authoritative firmware control bit.
    pub fn is_open(&self) -> bool {
        self.control.values.open.enabled()
    }

    /// Reads host status or forwards a complete permitted downstream transaction.
    pub fn read<E>(
        &self,
        address: usize,
        out: &mut [u8],
        downstream: impl FnOnce(usize, &mut [u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        let status_base = self.status_base;
        if address >= status_base && address - status_base < WindowStatus::EXTENT {
            let mut status = RegisterWindow::<WindowStatus>::new(status_base);
            status.values.ready = ReadWordValues::new(u32::from(self.is_open()));
            return status.read(address, out).map_err(AccessError::Register);
        }
        self.check_downstream(address, out.len())?;
        downstream(address, out).map_err(AccessError::Downstream)
    }

    /// Forwards a complete permitted host write; management and status remain protected.
    pub fn write<E>(
        &self,
        address: usize,
        source: &[u8],
        downstream: impl FnOnce(usize, &[u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        self.check_downstream(address, source.len())?;
        downstream(address, source).map_err(AccessError::Downstream)
    }

    fn check_downstream<E>(&self, address: usize, width: usize) -> Result<(), AccessError<E>> {
        let end = address.checked_add(width).ok_or(AccessError::Unmapped)?;
        let status_base = self.status_base;
        if (address < self.control.base() + WindowControl::EXTENT && end > self.control.base())
            || (address < status_base + WindowStatus::EXTENT && end > status_base)
        {
            return Err(AccessError::Denied);
        }
        if width == 0 || address < self.start || address >= self.extent || end > self.extent {
            return Err(AccessError::Unmapped);
        }
        if !self.is_open() {
            return Err(AccessError::Closed);
        }
        Ok(())
    }
}
