use super::{ChannelControl, Error};
use hardware_interface_runtime::RegisterAccess;

impl ChannelControl {
    pub(super) async fn channel_read_word<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
    ) -> Result<u32, Error<B::Error>> {
        read_word(
            bus,
            self.channel.checked_add(offset).ok_or(Error::Geometry)?,
        )
        .await
    }

    pub(super) async fn channel_write<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error>> {
        write(
            bus,
            self.channel.checked_add(offset).ok_or(Error::Geometry)?,
            bytes,
        )
        .await
    }
}

pub(super) async fn read_word<B: RegisterAccess + Sync>(
    bus: &B,
    offset: usize,
) -> Result<u32, Error<B::Error>> {
    let mut bytes = [0; 4];
    bus.read(
        u32::try_from(offset).map_err(|_| Error::Geometry)?,
        &mut bytes,
    )
    .await
    .map_err(Error::Bus)?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) async fn write<B: RegisterAccess + Sync>(
    bus: &B,
    offset: usize,
    bytes: &[u8],
) -> Result<(), Error<B::Error>> {
    bus.write(u32::try_from(offset).map_err(|_| Error::Geometry)?, bytes)
        .await
        .map_err(Error::Bus)
}
