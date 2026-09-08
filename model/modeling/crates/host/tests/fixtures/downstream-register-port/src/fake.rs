use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use pdma_model_host::adapter::abi::{
    PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim, PdmaIrqAccess, PdmaIrqCoordinator,
    PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegionRequest, PdmaRegisterAccessError,
};
use std::error::Error;
use std::sync::atomic::Ordering;

mod fates;
mod helpers;
mod state;
pub(crate) use fates::verify_fates;
pub use helpers::message;
use helpers::{crossing, lock, validate_read, validate_write};
pub use state::{FakeAccess, FakeSnapshot};

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

impl MemoryAccess for FakeAccess {
    fn read(
        &self,
        address: MemoryAddress,
        _: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let state = lock(&self.state);
        if destination.is_empty()
            && state
                .mappings
                .iter()
                .any(|mapping| mapping.base() == address && mapping.length() == 0)
        {
            return Ok(());
        }
        let bytes = state
            .bytes
            .get(&address)
            .ok_or(MemoryAccessError::Unmapped { address })?;
        if destination.len() > bytes.len() {
            return Err(crossing(address, destination.len(), &state));
        }
        destination.copy_from_slice(&bytes[..destination.len()]);
        Ok(())
    }
    fn write(
        &self,
        address: MemoryAddress,
        _: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let mut state = lock(&self.state);
        if source.is_empty()
            && state
                .mappings
                .iter()
                .any(|mapping| mapping.base() == address && mapping.length() == 0)
        {
            return Ok(());
        }
        let bytes = state
            .bytes
            .get_mut(&address)
            .ok_or(MemoryAccessError::Unmapped { address })?;
        if source.len() > bytes.len() {
            return Err(MemoryAccessError::Unmapped { address });
        }
        bytes[..source.len()].copy_from_slice(source);
        Ok(())
    }
}

impl RegisterAccess for FakeAccess {
    type Error = PdmaRegisterAccessError;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        if let Some(error) = lock(&self.next_register_error).take() {
            return Err(error);
        }
        validate_read(offset, response.len())?;
        let state = lock(&self.state);
        if let Some(bytes) = state.registers.get(&offset) {
            response.copy_from_slice(bytes);
        } else {
            response.fill(0);
        }
        Ok(())
    }
    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        if let Some(error) = lock(&self.next_register_error).take() {
            return Err(error);
        }
        if self.pending_register.swap(false, Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        validate_write(offset, request)?;
        let mut state = lock(&self.state);
        state.registers.insert(offset, request.to_vec());
        if offset == 0x164 {
            let epoch = state.registers.get(&0x100).cloned().unwrap_or(vec![0; 8]);
            let queue = state.registers.get(&0x108).cloned().unwrap_or(vec![0; 8]);
            state.registers.insert(0x168, 1_u32.to_le_bytes().to_vec());
            state.registers.insert(0x188, epoch);
            state.registers.insert(0x190, queue);
        }
        Ok(())
    }
}

impl PdmaMemoryProvisioning for FakeAccess {
    type Error = PdmaProvisioningError;
    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        if self.fail_allocation.swap(false, Ordering::SeqCst) {
            return Err(PdmaProvisioningError::Provisioning(
                PdmaProvisioningCause::StorageExhausted,
            ));
        }
        self.allocate(request)
    }
}

impl PdmaExactInterruptClaim for FakeAccess {
    type Error = PdmaClaimError;
    async fn wait_interrupt_ready(&self) -> Result<bool, Self::Error> {
        Ok(true)
    }
    async fn claim_interrupt_exact(&self) -> Result<PdmaClaimResult, Self::Error> {
        if self.pending_claim.swap(false, Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        if let Some(error) = lock(&self.next_claim_error).take() {
            return Err(error);
        }
        Ok(lock(&self.state)
            .notifications
            .pop_front()
            .map_or(PdmaClaimResult::Empty, |notification| {
                PdmaClaimResult::Notification { notification }
            }))
    }
}

impl PdmaIrqAccess for FakeAccess {
    fn irq_coordinator(&self) -> &PdmaIrqCoordinator {
        &self.irq
    }
}

#[cfg(test)]
mod tests;
