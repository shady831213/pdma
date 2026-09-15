use crate::{AccessError, Hardware, ManagementBus, Shared};
use hardware_interface_runtime::RegisterAccess;
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
            .access(|Hardware { dma, window }| {
                window.read(address, bytes, |address, bytes| dma.read(address, bytes))
            })
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), AccessError> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.shared
            .access(|Hardware { dma, window }| {
                window.write(address, bytes, |address, bytes| dma.write(address, bytes))
            })
            .await
    }
}

impl<M: Send> RegisterAccess for ManagementBus<M> {
    type Error = AccessError;

    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                hardware
                    .window
                    .management_read(address, bytes)
                    .map_err(AccessError::Register)
            })
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                hardware
                    .window
                    .management_write(address, bytes)
                    .map_err(AccessError::Register)
            })
            .await
    }
}
