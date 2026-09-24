use super::{AccessError, Hardware, ManagementBus, Shared};
use hardware_interface_runtime::RegisterAccess;
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::memory::FunctionId;
use std::sync::Arc;

/// Host work MMIO, checked by the hardware access window on every transaction.
#[derive(Debug)]
pub struct HostBus<M> {
    pub(super) shared: Arc<Shared<M>>,
    pub(super) function: FunctionId,
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
                     windows,
                     firmware,
                     control,
                     contexts,
                 }| {
                    if let Some(contexts) = contexts {
                        let result = contexts
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .host_read(address, bytes);
                        match result {
                            Err(RegisterAccessError::OutsideWindow { .. }) => {}
                            Err(RegisterAccessError::ReadForbidden { .. }) => {
                                return Err(AccessError::Denied);
                            }
                            result => return result.map_err(AccessError::Register),
                        }
                    }
                    let result = control
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .host_read(self.function, address, bytes);
                    match result {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Control),
                    }
                    match firmware.read(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => windows
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .for_function(self.function)
                            .read(address, bytes, |offset, bytes| {
                                dma.read(
                                    offset
                                        .checked_sub(dma_base)
                                        .ok_or(pdma_model_dma::Error::Unmapped)?,
                                    bytes,
                                )
                            })
                            .map_err(AccessError::from),
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
                     windows,
                     firmware,
                     control,
                     contexts,
                 }| {
                    if let Some(contexts) = contexts {
                        let result = contexts
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .host_write(address, bytes);
                        match result {
                            Err(RegisterAccessError::OutsideWindow { .. }) => {}
                            Err(RegisterAccessError::WriteForbidden { .. }) => {
                                return Err(AccessError::Denied);
                            }
                            result => return result.map_err(AccessError::Register),
                        }
                    }
                    let result = control
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .host_write(self.function, address, bytes);
                    match result {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Control),
                    }
                    match firmware.host_write(address, bytes) {
                        Err(RegisterAccessError::OutsideWindow { .. }) => windows
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .for_function(self.function)
                            .write(address, bytes, |offset, bytes| {
                                dma.write(
                                    offset
                                        .checked_sub(dma_base)
                                        .ok_or(pdma_model_dma::Error::Unmapped)?,
                                    bytes,
                                )
                            })
                            .map_err(AccessError::from),
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
                if let Some(contexts) = &hardware.contexts {
                    let result = contexts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .management_read(address, bytes);
                    match result {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Register),
                    }
                }
                let result = hardware
                    .control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .firmware_read(address, bytes);
                match result {
                    Err(RegisterAccessError::OutsideWindow { .. }) => {}
                    result => return result.map_err(AccessError::Control),
                }
                match register_dispatch!(
                    hardware.firmware.read(address, bytes),
                    hardware
                        .windows
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .management_read(address, bytes),
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
                if let Some(contexts) = &hardware.contexts {
                    let windows = hardware
                        .windows
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let result = contexts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .management_write((address, bytes), (&mut hardware.dma, &windows));
                    drop(windows);
                    match result {
                        Err(RegisterAccessError::OutsideWindow { .. }) => {}
                        result => return result.map_err(AccessError::Register),
                    }
                }
                let result = hardware
                    .control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .firmware_write(address, bytes);
                match result {
                    Err(RegisterAccessError::OutsideWindow { .. }) => {}
                    result => return result.map_err(AccessError::Control),
                }
                match register_dispatch!(
                    hardware.firmware.management_write(address, bytes),
                    hardware
                        .windows
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .management_write(address, bytes),
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
