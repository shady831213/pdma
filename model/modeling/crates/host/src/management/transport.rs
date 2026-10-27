use super::Management;
use crate::Error;
use hardware_interface_register::RegisterAccess;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_software::ring::SoftRingError;

impl<B: RegisterAccess + Sync, M: Sync> Management<B, M> {
    pub(super) async fn read<E>(&self, offset: usize) -> Result<u32, Error<B::Error, E>> {
        let mut bytes = [0; 4];
        self.bus
            .read(address(self.addresses.control, offset)?, &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn write<E>(
        &self,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), Error<B::Error, E>> {
        self.bus
            .write(address(self.addresses.control, offset)?, bytes)
            .await
            .map_err(Error::Bus)
    }

    pub(super) async fn kick<E>(&self) -> Result<(), Error<B::Error, E>> {
        self.write(ControlRegisters::REQUEST.offset(), &1_u32.to_le_bytes())
            .await
    }
}

fn address<B, E>(base: u32, offset: usize) -> Result<u32, Error<B, E>> {
    base.checked_add(u32::try_from(offset).map_err(|_| Error::Geometry)?)
        .ok_or(Error::Geometry)
}

pub(super) fn ring_error<B, E>(error: SoftRingError<E, E>) -> Error<B, E> {
    match error {
        SoftRingError::Cursor(error) | SoftRingError::Memory(error) => Error::Memory(error),
        SoftRingError::Codec(error) => Error::Codec(error),
        SoftRingError::Ring(error) => Error::Ring(error),
        SoftRingError::Busy => Error::RingBusy,
        SoftRingError::Idle => Error::RingIdle,
        SoftRingError::CursorChanged => Error::RingCursorChanged,
    }
}
