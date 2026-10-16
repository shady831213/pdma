use super::{Channel, Dma, Error};
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::dma::{ChannelId, DmaControl, ResetValues};

impl<M, S> Dma<M, S> {
    /// Reads a register and applies the owning channel's immediate side effects.
    pub fn read(&mut self, offset: usize, destination: &mut [u8]) -> Result<(), Error> {
        register_dispatch!(
            self.discovery
                .read(offset, destination)
                .map_err(RegisterAccessError::widen),
            self.control
                .read(offset, destination)
                .map_err(RegisterAccessError::widen),
            self.read_channels(offset, destination),
            self.read_admin(offset, destination),
        )
        .map_err(Error::from)
    }

    /// Writes a register and applies the owning channel's immediate side effects.
    pub fn write(&mut self, offset: usize, source: &[u8]) -> Result<(), Error> {
        let result = register_dispatch!(
            self.discovery
                .write(offset, source)
                .map_err(RegisterAccessError::widen),
            self.write_control(offset, source),
            self.write_channels(offset, source),
            self.write_admin(offset, source),
        )
        .map_err(Error::from);
        if result.is_ok() {
            let id = self
                .layout
                .channels()
                .locate(offset)
                .map(|(index, _)| pdma_model_contract::dma::ChannelId::new(index));
            if let Some(id) = id {
                self.observe_channel(id);
            }
        }
        result
    }

    pub(super) fn write_control(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let maintenance = self.maintenance_interlock(None);
        let resetting = self.resetting();
        let mut requested = false;
        self.control.write_with(
            address,
            source,
            |offset, _, next| {
                if offset == DmaControl::RESET.offset() && next.reset.request() {
                    maintenance?;
                }
                if resetting && offset != DmaControl::RESET.offset() {
                    return Err(Error::ResetInProgress);
                }
                Ok(())
            },
            |offset, next| {
                if offset == DmaControl::RESET.offset() {
                    requested = next.reset.request();
                    next.reset = ResetValues::new(u32::from(resetting));
                }
            },
        )?;
        if requested {
            self.request_reset();
        }
        Ok(())
    }

    pub(super) fn read_channels(
        &mut self,
        address: usize,
        destination: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let (channel, offset, _) = self.locate_channel(address)?;
        register_dispatch!(
            channel.read(offset, destination),
            Err(RegisterAccessError::InvalidOffset { offset }),
        )
    }

    pub(super) fn write_channels(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let halted = self.halted();
        let resetting = self.resetting();
        let (channel, offset, maintenance) = self.locate_channel(address)?;
        if resetting {
            return Err(RegisterAccessError::Other(Error::ResetInProgress));
        }
        if halted {
            return Err(RegisterAccessError::Other(Error::Halted));
        }
        register_dispatch!(
            channel.write(offset, source, maintenance),
            Err(RegisterAccessError::InvalidOffset { offset }),
        )
    }

    pub(super) fn read_admin(
        &mut self,
        address: usize,
        destination: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let (channel, offset, _) = self.locate_admin(address)?;
        channel
            .admin
            .read(offset, destination)
            .map_err(RegisterAccessError::widen)
    }

    pub(super) fn write_admin(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let (channel, offset, maintenance) = self.locate_admin(address)?;
        channel.write_admin(offset, source, maintenance)?;
        self.release_invalid_grant();
        Ok(())
    }

    fn locate_channel(
        &mut self,
        address: usize,
    ) -> Result<(&mut Channel, usize, bool), RegisterAccessError<Error>> {
        let (index, offset) = self
            .layout
            .channels()
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        let maintenance = self
            .maintenance_interlock(Some(ChannelId::new(index)))
            .is_err();
        let channel = self
            .channels
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        Ok((channel, offset, maintenance))
    }

    fn locate_admin(
        &mut self,
        address: usize,
    ) -> Result<(&mut Channel, usize, bool), RegisterAccessError<Error>> {
        let admin = self.layout.admin();
        let (index, offset) = admin
            .locate(address)
            .and_then(|(_, offset)| admin.child().channels().locate(offset))
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        let maintenance = self
            .maintenance_interlock(Some(ChannelId::new(index)))
            .is_err();
        let channel = self
            .channels
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        Ok((channel, offset, maintenance))
    }
}
