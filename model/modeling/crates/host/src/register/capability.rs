use super::{
    PdmaClaimResult, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest, PdmaSubmissionBatch,
};
use pdma_model_contract::binding::QueueActivation;
use std::error::Error;

/// Host-owned logical PDMA mapping allocator and mutation-free preparer.
pub trait PdmaMemoryProvisioning: Clone + Send + Sync {
    /// Opaque proposal consumed by this capability's closed batches.
    type Proposal: PdmaPreparedProposal;
    /// Typed allocation/preparation failure.
    type Error: Error + Send + Sync + 'static;

    /// Allocates and commits exactly one logical region.
    fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> impl Future<Output = Result<PdmaMappedRegion, Self::Error>> + Send;

    /// Prepares logical regions without changing mappings, bytes, or generation.
    fn prepare_regions(
        &self,
        requests: &[PdmaRegionRequest],
    ) -> impl Future<Output = Result<Self::Proposal, Self::Error>> + Send;
}

/// Two closed all-or-none PDMA batches.
pub trait PdmaBatchAccess: PdmaMemoryProvisioning {
    /// Typed batch failure.
    type BatchError: Error + Send + Sync + 'static;

    /// Commits prepared queue rings and activation as the sole proposal commit.
    fn bootstrap_queue(
        &self,
        proposal: Self::Proposal,
        activation: QueueActivation,
    ) -> impl Future<Output = Result<[u8; 72], Self::BatchError>> + Send;

    /// Commits optional prepared indirect storage, writes and producer publication.
    fn submit_queue(
        &self,
        request: PdmaSubmissionBatch<Self::Proposal>,
    ) -> impl Future<Output = Result<[u8; 72], Self::BatchError>> + Send;
}

/// PDMA-specific readiness plus atomic exact interrupt claim.
pub trait PdmaExactInterruptClaim: Clone + Send + Sync {
    /// Typed readiness/claim failure.
    type Error: Error + Send + Sync + 'static;

    /// Waits without consuming until the interrupt source is ready or closed.
    fn wait_interrupt_ready(&self) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Waits for readiness, then captures command/result/all responses in one compound commit poll.
    fn claim_interrupt_exact(
        &self,
    ) -> impl Future<Output = Result<PdmaClaimResult, Self::Error>> + Send;
}
