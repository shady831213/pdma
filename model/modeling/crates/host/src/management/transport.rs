use super::Management;
use crate::Error;
use hardware_interface_runtime::{MemoryAccessError, RegisterAccess};
use pdma_model_contract::ControlRegisters;
use pdma_model_software::ring::SoftRingError;

impl<B: RegisterAccess + Sync, M: Sync> Management<B, M> {
    pub(super) async fn read(&self, offset: usize) -> Result<u32, Error<B::Error>> {
        let mut bytes = [0; 4];
        self.bus
            .read(address(self.addresses.control, offset)?, &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Error<B::Error>> {
        self.bus
            .write(address(self.addresses.control, offset)?, bytes)
            .await
            .map_err(Error::Bus)
    }

    pub(super) async fn kick(&self) -> Result<(), Error<B::Error>> {
        self.write(ControlRegisters::REQUEST.offset(), &1_u32.to_le_bytes())
            .await
    }
}

fn address<B>(base: u32, offset: usize) -> Result<u32, Error<B>> {
    base.checked_add(u32::try_from(offset).map_err(|_| Error::Geometry)?)
        .ok_or(Error::Geometry)
}

pub(super) const fn ring_error<B>(error: SoftRingError<MemoryAccessError>) -> Error<B> {
    match error {
        SoftRingError::Cursor(error) | SoftRingError::Memory(error) => Error::Memory(error),
        SoftRingError::Codec(error) => Error::Codec(error),
        SoftRingError::Ring(error) => Error::Ring(error),
        SoftRingError::Busy => Error::RingBusy,
        SoftRingError::Idle => Error::RingIdle,
        SoftRingError::CursorChanged => Error::RingCursorChanged,
    }
}
