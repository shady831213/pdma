use super::Management;
use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{CONTROL_CHANNEL_BASE, ControlRingRegisters, RingPointers};

impl<B: RegisterAccess + Sync, M: Sync> Management<B, M> {
    pub(super) async fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Error<B::Error>> {
        let address = CONTROL_CHANNEL_BASE
            .checked_add(u32::try_from(offset).map_err(|_| Error::Geometry)?)
            .ok_or(Error::Geometry)?;
        self.owner
            .scope(self.bus.write(address, bytes))
            .await
            .map_err(Error::Cpu)?
            .map_err(Error::Bus)
    }

    pub(super) async fn read(&self, offset: usize) -> Result<u32, Error<B::Error>> {
        let address = CONTROL_CHANNEL_BASE
            .checked_add(u32::try_from(offset).map_err(|_| Error::Geometry)?)
            .ok_or(Error::Geometry)?;
        let mut bytes = [0; 4];
        self.owner
            .scope(self.bus.read(address, &mut bytes))
            .await
            .map_err(Error::Cpu)?
            .map_err(Error::Bus)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn pointers(&self, offset: usize) -> Result<RingPointers, Error<B::Error>> {
        Ok(RingPointers {
            producer: self
                .read(offset + ControlRingRegisters::PRODUCER.offset())
                .await?,
            consumer: self
                .read(offset + ControlRingRegisters::CONSUMER.offset())
                .await?,
        })
    }
}
