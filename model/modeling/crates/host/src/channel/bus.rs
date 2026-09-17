use crate::Error;
use hardware_interface_runtime::{CpuOwner, RegisterAccess};
use pdma_model_contract::{ChannelId, DeviceControl, DeviceRegisters};
use pdma_model_software::ring::{RegisterRingIo, RingCursorRegisters};
use std::future::Future;

#[derive(Debug)]
pub(super) struct RegisterPort<B> {
    bus: B,
    owner: CpuOwner,
    channel_base: usize,
}

impl<B: RegisterAccess + Send + Sync> RegisterPort<B> {
    pub(super) async fn new(
        bus: B,
        owner: CpuOwner,
        id: ChannelId,
    ) -> Result<Self, Error<B::Error>> {
        let mut port = Self {
            bus,
            owner,
            channel_base: 0,
        };
        let available = port
            .read_word(DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset())
            .await?;
        if id.index() >= usize::try_from(available).map_err(|_| Error::Geometry)? {
            return Err(Error::Channel {
                requested: id,
                available,
            });
        }
        port.channel_base = id.register_offset().ok_or(Error::Geometry)?;
        Ok(port)
    }

    pub(super) fn validate_active(&self) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)
    }

    pub(super) fn ring_io<'a, M>(
        &'a self,
        memory: &'a M,
        registers: RingCursorRegisters,
    ) -> Result<RegisterRingIo<'a, B, M>, Error<B::Error>> {
        self.validate_active()?;
        let base = u32::try_from(self.channel_base).map_err(|_| Error::Geometry)?;
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

    pub(super) async fn channel_read_word(&self, offset: usize) -> Result<u32, Error<B::Error>> {
        self.read_word(
            self.channel_base
                .checked_add(offset)
                .ok_or(Error::Geometry)?,
        )
        .await
    }

    pub(super) async fn channel_write(
        &self,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error>> {
        self.write(
            self.channel_base
                .checked_add(offset)
                .ok_or(Error::Geometry)?,
            bytes,
        )
        .await
    }

    pub(super) async fn read_word(&self, offset: usize) -> Result<u32, Error<B::Error>> {
        self.validate_active()?;
        let mut bytes = [0; 4];
        self.checked_bus(self.bus.read(
            u32::try_from(offset).map_err(|_| Error::Geometry)?,
            &mut bytes,
        ))
        .await?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Error<B::Error>> {
        self.validate_active()?;
        self.checked_bus(
            self.bus
                .write(u32::try_from(offset).map_err(|_| Error::Geometry)?, bytes),
        )
        .await
    }

    async fn checked_bus(
        &self,
        operation: impl Future<Output = Result<(), B::Error>> + Send,
    ) -> Result<(), Error<B::Error>> {
        self.owner
            .scope(operation)
            .await
            .map_err(Error::Cpu)?
            .map_err(Error::Bus)
    }
}
