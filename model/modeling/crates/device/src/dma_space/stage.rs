use pdma_model_contract::abi::DmaRegion;

/// Read-only proposal for one atomic DMA allocation batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaStage {
    pub(super) generation: u64,
    pub(super) next_iova: u64,
    pub(super) regions: Vec<DmaRegion>,
}

impl DmaStage {
    /// Returns the allocator generation observed when this proposal was created.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the proposed regions without installing them.
    pub fn regions(&self) -> &[DmaRegion] {
        &self.regions
    }
}
