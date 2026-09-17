use super::{ChannelControl, Error};
use hardware_interface_runtime::RegisterAccess;
use std::future::Future;

impl ChannelControl {
    pub(super) async fn channel_read_word<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
    ) -> Result<u32, Error<B::Error>> {
        self.read_word(
            bus,
            self.channel_base
                .checked_add(offset)
                .ok_or(Error::Geometry)?,
        )
        .await
    }

    pub(super) async fn channel_write<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error>> {
        self.write(
            bus,
            self.channel_base
                .checked_add(offset)
                .ok_or(Error::Geometry)?,
            bytes,
        )
        .await
    }

    pub(super) async fn read_word<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
    ) -> Result<u32, Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        let mut bytes = [0; 4];
        self.checked_bus(bus.read(
            u32::try_from(offset).map_err(|_| Error::Geometry)?,
            &mut bytes,
        ))
        .await?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn write<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        self.checked_bus(bus.write(u32::try_from(offset).map_err(|_| Error::Geometry)?, bytes))
            .await
    }

    async fn checked_bus<E>(
        &self,
        operation: impl Future<Output = Result<(), E>> + Send,
    ) -> Result<(), Error<E>> {
        self.owner
            .scope(operation)
            .await
            .map_err(Error::Cpu)?
            .map_err(Error::Bus)
    }
}
