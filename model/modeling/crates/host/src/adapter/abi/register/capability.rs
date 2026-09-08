use super::{PdmaMappedRegion, PdmaRegionRequest};
use pdma_model_contract::semantic::DeviceEpoch;
use std::error::Error;

/// Host-owned logical PDMA mapping allocator.
pub trait PdmaMemoryProvisioning: Clone + Send + Sync {
    /// Typed allocation failure.
    type Error: Error + Send + Sync + 'static;

    /// Binds register accesses and interrupt consumption to one device epoch.
    #[must_use]
    fn at_epoch(self, epoch: DeviceEpoch) -> Self;

    /// Allocates and commits exactly one logical region.
    fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> impl Future<Output = Result<PdmaMappedRegion, Self::Error>> + Send;
}
