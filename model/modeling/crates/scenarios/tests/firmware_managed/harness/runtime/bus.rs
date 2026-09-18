use super::{AccessError, Hardware, ManagementBus, Shared};
use hardware_interface_runtime::RegisterAccess;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use std::sync::Arc;

/// Host work MMIO, checked by the hardware access window on every transaction.
#[derive(Debug)]
pub struct HostBus<M> {
    pub(super) shared: Arc<Shared<M>>,
    pub(super) function: pdma_model_contract::FunctionId,
    pub(super) addresses: pdma_model_software::control::ManagementAddresses,
}

impl<M> Clone for HostBus<M> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            function: self.function,
            addresses: self.addresses,
        }
    }
}

impl<M> HostBus<M> {
    /// Supplies the software program's platform register locations.
    pub const fn addresses(&self) -> pdma_model_software::control::ManagementAddresses {
        self.addresses
    }
}

impl<M: Send> RegisterAccess for HostBus<M> {
    type Error = AccessError;

    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), AccessError> {
        let dma_base =
            usize::try_from(self.shared.addresses.dma).map_err(|_| AccessError::Unmapped)?;
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.shared
            .access(
                |Hardware {
                     dma,
                     window,
                     firmware,
                     control,
                 }| {
                    match control.host_read(self.function, address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Control),
                    }
                    match firmware.read(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => window
                            .for_function(self.function)
                            .read(address, bytes, |address, bytes| {
                                dma.read(
                                    address
                                        .checked_sub(dma_base)
                                        .ok_or(pdma_model_dma::Error::Unmapped)?,
                                    bytes,
                                )
                            }),
                        result => result.map_err(AccessError::Register),
                    }
                },
            )
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), AccessError> {
        let dma_base =
            usize::try_from(self.shared.addresses.dma).map_err(|_| AccessError::Unmapped)?;
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.shared
            .access(
                |Hardware {
                     dma,
                     window,
                     firmware,
                     control,
                 }| {
                    match control.host_write(self.function, address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Control),
                    }
                    match firmware.host_write(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => window
                            .for_function(self.function)
                            .write(address, bytes, |address, bytes| {
                                dma.write(
                                    address
                                        .checked_sub(dma_base)
                                        .ok_or(pdma_model_dma::Error::Unmapped)?,
                                    bytes,
                                )
                            }),
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
        let dma_base = usize::try_from(self.0.addresses.dma).map_err(|_| AccessError::Unmapped)?;
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                match hardware.control.firmware_read(address, bytes) {
                    Err(RegisterAccessError::OutsideWindow { .. }) => {}
                    result => return result.map_err(AccessError::Control),
                }
                match register_dispatch!(
                    hardware.firmware.read(address, bytes),
                    hardware.window.management_read(address, bytes),
                ) {
                    Err(RegisterAccessError::OutsideWindow { .. }) => hardware
                        .dma
                        .read(
                            address.checked_sub(dma_base).ok_or(AccessError::Unmapped)?,
                            bytes,
                        )
                        .map_err(AccessError::Downstream),
                    result => result.map_err(AccessError::Register),
                }
            })
            .await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let dma_base = usize::try_from(self.0.addresses.dma).map_err(|_| AccessError::Unmapped)?;
        let address = usize::try_from(address).map_err(|_| AccessError::Unmapped)?;
        self.0
            .access(|hardware| {
                match hardware.control.firmware_write(address, bytes) {
                    Err(RegisterAccessError::OutsideWindow { .. }) => {}
                    result => return result.map_err(AccessError::Control),
                }
                match register_dispatch!(
                    hardware.firmware.management_write(address, bytes),
                    hardware.window.management_write(address, bytes),
                ) {
                    Err(RegisterAccessError::OutsideWindow { .. }) => hardware
                        .dma
                        .write(
                            address.checked_sub(dma_base).ok_or(AccessError::Unmapped)?,
                            bytes,
                        )
                        .map_err(AccessError::Downstream),
                    result => result.map_err(AccessError::Register),
                }
            })
            .await
    }
}
