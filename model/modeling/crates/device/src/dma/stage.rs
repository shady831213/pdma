use crate::dma::MappedDmaRegion;

/// Read-only proposal for one atomic DMA allocation batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdmaMemoryStage {
    pub(super) generation: u64,
    pub(super) next_address: u64,
    pub(super) regions: Vec<MappedDmaRegion>,
}

impl PdmaMemoryStage {
    /// Returns the allocator generation observed when this proposal was created.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the proposed regions without installing them.
    pub fn regions(&self) -> &[MappedDmaRegion] {
        &self.regions
    }
}
