//! Test-only mapping fault selected by a ring's programmed base register.

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess,
    SharedMemoryAccess,
};
use std::sync::{Arc, Mutex};

/// Memory view rejecting writes to the first entry of a software-configured ring.
#[derive(Debug)]
pub(super) struct UnmappedRing {
    memory: SharedMemoryAccess,
    address: Mutex<Option<MemoryAddress>>,
}

impl UnmappedRing {
    /// Uses normal backing until the paired bus observes the selected ring base.
    pub(super) const fn new(memory: SharedMemoryAccess) -> Self {
        Self {
            memory,
            address: Mutex::new(None),
        }
    }
}

impl MemoryAccess for UnmappedRing {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if *self
            .address
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            == Some(address)
        {
            return Err(MemoryAccessError::Unmapped { address });
        }
        self.memory.write(address, alignment, source)
    }
}

/// Observes actual software-selected addresses without changing register traffic.
#[derive(Debug)]
pub(super) struct FaultBus<B> {
    /// Real register transport.
    pub(super) bus: B,
    /// Generated offset of the ring base whose mapping should fail.
    pub(super) base_register: Option<u32>,
    /// Faulted view shared by the injected memory endpoint.
    pub(super) memory: Arc<UnmappedRing>,
}

impl<B: RegisterAccess + Send + Sync> RegisterAccess for FaultBus<B> {
    type Error = B::Error;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.bus.read(offset, response).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.bus.write(offset, request).await?;
        if Some(offset) == self.base_register {
            if let Ok(bytes) = <[u8; 8]>::try_from(request) {
                *self
                    .memory
                    .address
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(MemoryAddress::new(u64::from_le_bytes(bytes)));
            }
        }
        Ok(())
    }
}
