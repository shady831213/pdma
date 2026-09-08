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

/// PDMA-specific readiness plus a host-owned exact interrupt claim.
pub trait PdmaExactInterruptClaim: Clone + Send + Sync {
    /// Typed readiness/claim failure.
    type Error: Error + Send + Sync + 'static;

    /// Waits without consuming until the interrupt source is ready or closed.
    fn wait_interrupt_ready(&self) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Reads one notification, retaining acknowledged consumption across cancellation.
    fn claim_interrupt_exact(
        &self,
    ) -> impl Future<Output = Result<PdmaClaimResult, Self::Error>> + Send;
}
