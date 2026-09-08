use super::{PdmaClaimResult, PdmaMappedRegion, PdmaRegionRequest};
use std::error::Error;

/// Host-owned logical PDMA mapping allocator.
pub trait PdmaMemoryProvisioning: Clone + Send + Sync {
    /// Typed allocation failure.
    type Error: Error + Send + Sync + 'static;

    /// Allocates and commits exactly one logical region.
    fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> impl Future<Output = Result<PdmaMappedRegion, Self::Error>> + Send;
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
