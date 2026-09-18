use super::{ChannelControl, Error};
use crate::ring::{RegisterRingIo, RingCursorRegisters};
use hardware_interface_runtime::{CpuOwner, RegisterAccess};
use pdma_model_contract::ChannelId;

#[derive(Debug)]
pub(super) struct RegisterPort<B> {
    pub(super) bus: B,
    pub(super) control: ChannelControl,
}

impl<B: RegisterAccess + Send + Sync> RegisterPort<B> {
    pub(super) async fn new(
        bus: B,
        owner: CpuOwner,
        id: ChannelId,
        dma_base: u32,
    ) -> Result<Self, Error<B::Error>> {
        let control = ChannelControl::discover_owned(&bus, owner, id, dma_base).await?;
        Ok(Self { bus, control })
    }

    pub(super) fn validate_active(&self) -> Result<(), Error<B::Error>> {
        self.control.owner.validate_active().map_err(Error::Cpu)
    }

    pub(super) fn ring_io<'a, M>(
        &'a self,
        memory: &'a M,
        registers: RingCursorRegisters,
    ) -> Result<RegisterRingIo<'a, B, M>, Error<B::Error>> {
        self.validate_active()?;
        let base = u32::try_from(self.control.channel_base).map_err(|_| Error::Geometry)?;
        let registers = RingCursorRegisters {
            producer: base
                .checked_add(registers.producer)
                .ok_or(Error::Geometry)?,
            consumer: base
                .checked_add(registers.consumer)
                .ok_or(Error::Geometry)?,
        };
        Ok(RegisterRingIo::new(&self.bus, memory, registers))
    }

    pub(super) async fn channel_write(
        &self,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error>> {
        self.control.channel_write(&self.bus, offset, bytes).await
    }
}
