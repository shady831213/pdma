use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use pdma_model_contract::semantic::QueueActivation;
use pdma_model_host::adapter::abi::{
    PdmaBatchAccess, PdmaBatchError, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim,
    PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegionRequest, PdmaRegisterAccessError, PdmaSubmissionBatch,
};
use std::error::Error;
use std::sync::atomic::Ordering;

mod fates;
mod helpers;
mod state;
pub(crate) use fates::verify_fates;
pub use helpers::{activation, message};
use helpers::{claim_observations, crossing, lock, response, validate_read, validate_write};
pub use state::{FakeAccess, FakeProposal, FakeSnapshot};

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
        lock(&self.state).registers.insert(offset, request.to_vec());
        Ok(())
    }
}

impl PdmaMemoryProvisioning for FakeAccess {
    type Proposal = FakeProposal;
    type Error = PdmaProvisioningError;
    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        let proposal = self.prepare(&[request])?;
        if self.fail_allocation.swap(false, Ordering::SeqCst) {
            return Err(PdmaProvisioningError::Provisioning(
                PdmaProvisioningCause::StorageExhausted,
            ));
        }
        self.commit(proposal)?
            .into_iter()
            .next()
            .ok_or(PdmaProvisioningError::InvalidProjection)
    }
    async fn prepare_regions(
        &self,
        requests: &[PdmaRegionRequest],
    ) -> Result<Self::Proposal, Self::Error> {
        self.prepare(requests)
    }
}

impl PdmaBatchAccess for FakeAccess {
    type BatchError = PdmaBatchError;
    async fn bootstrap_queue(
        &self,
        proposal: Self::Proposal,
        activation: QueueActivation,
    ) -> Result<pdma_model_contract::abi::mmio::QueueResponseRegistersRaw, Self::BatchError> {
        if let Some(error) = lock(&self.next_batch_error).take() {
            return Err(error);
        }
        self.commit(proposal)
            .map_err(PdmaBatchError::Provisioning)?;
        lock(&self.state).queue_staged = true;
        response(
            activation.queue().epoch().raw(),
            activation.queue().queue_id().raw(),
            0,
        )
    }
    async fn submit_queue(
        &self,
        request: PdmaSubmissionBatch<Self::Proposal>,
    ) -> Result<pdma_model_contract::abi::mmio::QueueResponseRegistersRaw, Self::BatchError> {
        if let Some(error) = lock(&self.next_batch_error).take() {
            return Err(error);
        }
        let (queue, producer, descriptor, indirect) = request.into_parts();
        if let Some((proposal, write)) = indirect {
            self.commit(proposal)
                .map_err(PdmaBatchError::Provisioning)?;
            lock(&self.state)
                .bytes
                .insert(write.address(), write.bytes().to_vec());
            lock(&self.state).indirect_staged = true;
        }
        let mut state = lock(&self.state);
        state
            .bytes
            .insert(descriptor.address(), descriptor.bytes().to_vec());
        state.submission_committed = true;
        state.retirement = true;
        drop(state);
        response(
            queue.epoch().raw(),
            queue.queue_id().raw(),
            producer.raw() + 1,
        )
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
        Ok(lock(&self.state).notifications.pop_front().map_or(
            PdmaClaimResult::Empty,
            |notification| PdmaClaimResult::Notification {
                notification,
                observations: claim_observations(),
            },
        ))
    }
}

#[cfg(test)]
mod tests;
