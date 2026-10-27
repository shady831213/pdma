use super::monitor::Monitor;
use crate::Error;
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::control::{ControlRequest, ControlResponse, ControlResponseStatus};
mod maintenance;
pub(super) use maintenance::Pending;
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::context::{
    ContextAttributeRegisters, ContextEntryRegisters, ContextManagementRegisters,
    ContextTableRegisters,
};
use pdma_model_contract::ring::RingError;

pub(super) struct Contexts {
    base: u32,
    count: usize,
    channel_count: usize,
}

impl Contexts {
    pub(super) async fn discover<B: RegisterAccess + Sync, M>(
        bus: &B,
        base: u32,
        channel_count: usize,
    ) -> Result<Self, Error<B::Error, M>> {
        let mut table = Self {
            base,
            count: 0,
            channel_count,
        };
        table.count = usize::try_from(
            table
                .read(bus, ContextManagementRegisters::CAPACITY.offset())
                .await?,
        )
        .map_err(|_| Error::Ring(RingError::Geometry))?;
        Ok(table)
    }

    async fn create<B: RegisterAccess + Sync, M>(
        &self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        (request, response): (&ControlRequest, &mut ControlResponse),
    ) -> Result<(), Error<B::Error, M>> {
        if request.attributes.validate().is_err() {
            response.status = ControlResponseStatus::InvalidAttributes;
            return Ok(());
        }
        for index in 0..self.count {
            let entry = self.entry(index)?;
            if self
                .read(bus, entry + ContextEntryRegisters::VALID.offset())
                .await?
                != 0
            {
                continue;
            }
            self.write(
                bus,
                (
                    entry + ContextEntryRegisters::OWNER.offset(),
                    request.function,
                ),
            )
            .await?;
            let attributes = entry + ContextEntryRegisters::ATTRIBUTES.offset();
            self.write(
                bus,
                (
                    attributes + ContextAttributeRegisters::ADDRESS_SPACE.offset(),
                    request.attributes.address_space.raw(),
                ),
            )
            .await?;
            self.write(
                bus,
                (
                    attributes + ContextAttributeRegisters::PASID.offset(),
                    request.attributes.pasid,
                ),
            )
            .await?;
            monitor.healthy().await?;
            self.write(bus, (entry + ContextEntryRegisters::VALID.offset(), 1))
                .await?;
            response.context =
                u32::try_from(index).map_err(|_| Error::Ring(RingError::Geometry))?;
            return Ok(());
        }
        response.status = ControlResponseStatus::NoResources;
        Ok(())
    }

    fn entry<E, M>(&self, index: usize) -> Result<usize, Error<E, M>> {
        ContextTableRegisters::for_channels(self.count, self.channel_count)
            .map_err(|_| Error::Ring(RingError::Geometry))?
            .entries()
            .offset(index)
            .ok_or(Error::Ring(RingError::Geometry))
    }

    async fn failed_channels<B: RegisterAccess + Sync, M>(
        &self,
        bus: &B,
        channels: impl Iterator<Item = ChannelId> + Send,
    ) -> Result<Vec<ChannelId>, Error<B::Error, M>> {
        let layout = ContextTableRegisters::for_channels(self.count, self.channel_count)
            .map_err(|_| Error::Ring(RingError::Geometry))?;
        let mut failed = Vec::new();
        let mut cached = None;
        for channel in channels {
            let index = channel.index() / 32;
            let bits = match cached {
                Some((previous, bits)) if previous == index => bits,
                Some(_) | None => {
                    let bits = self
                        .read(
                            bus,
                            layout
                                .failed()
                                .offset(index)
                                .ok_or(Error::Ring(RingError::Geometry))?,
                        )
                        .await?;
                    cached = Some((index, bits));
                    bits
                }
            };
            if bits & (1 << (channel.index() % 32)) != 0 {
                failed.try_reserve(1).map_err(Error::Allocation)?;
                failed.push(channel);
            }
        }
        Ok(failed)
    }

    fn address<E, M>(&self, offset: usize) -> Result<u32, Error<E, M>> {
        self.base
            .checked_add(u32::try_from(offset).map_err(|_| Error::Ring(RingError::Geometry))?)
            .ok_or(Error::Ring(RingError::Geometry))
    }

    async fn read<B: RegisterAccess + Sync, M>(
        &self,
        bus: &B,
        offset: usize,
    ) -> Result<u32, Error<B::Error, M>> {
        let mut bytes = [0; 4];
        bus.read(self.address(offset)?, &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u32::from_le_bytes(bytes))
    }

    async fn write<B: RegisterAccess + Sync, M>(
        &self,
        bus: &B,
        (offset, value): (usize, u32),
    ) -> Result<(), Error<B::Error, M>> {
        bus.write(self.address(offset)?, &value.to_le_bytes())
            .await
            .map_err(Error::Bus)
    }
}
