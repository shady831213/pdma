use super::{AccessError, Hardware, ManagementBus, Shared};
use hardware_interface_runtime::RegisterAccess;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use std::sync::Arc;

/// Host work MMIO, checked by the hardware access window on every transaction.
#[derive(Debug)]
pub struct HostBus<M> {
    pub(super) shared: Arc<Shared<M>>,
}

impl<M> Clone for HostBus<M> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<M: Send> RegisterAccess for HostBus<M> {
    type Error = AccessError;

    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), AccessError> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.shared
            .access(
                |Hardware {
                     dma,
                     window,
                     firmware,
                 }| {
                    match firmware.read(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {
                            window.read(address, bytes, |address, bytes| dma.read(address, bytes))
                        }
                        result => result.map_err(AccessError::Register),
                    }
                },
            )
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), AccessError> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.shared
            .access(
                |Hardware {
                     dma,
                     window,
                     firmware,
                 }| {
                    match firmware.host_write(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {
                            window.write(address, bytes, |address, bytes| dma.write(address, bytes))
                        }
                        result => result.map_err(AccessError::Register),
                    }
                },
            )
            .await
    }
}

impl<M: Send> RegisterAccess for ManagementBus<M> {
    type Error = AccessError;

    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                register_dispatch!(
                    hardware.firmware.read(address, bytes),
                    hardware.window.management_read(address, bytes),
                )
                .map_err(AccessError::Register)
            })
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                register_dispatch!(
                    hardware.firmware.management_write(address, bytes),
                    hardware.window.management_write(address, bytes),
                )
                .map_err(AccessError::Register)
            })
            .await
    }
}
