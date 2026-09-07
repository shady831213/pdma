use crate::ModelResult;
use crate::memory::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryStage};
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::BufferId;
use std::sync::Arc;

impl AsyncDeviceHandle {
    /// Returns the clone-shared runtime memory capability owned by this device core.
    pub fn memory_access(&self) -> Arc<hardware_interface_runtime::SharedMemory> {
        Arc::clone(&self.memory)
    }

    /// Allocates one raw device-visible DMA region.
    pub async fn allocate_memory_region(&self, len: usize) -> ModelResult<MappedDmaRegion> {
        self.shared
            .backend
            .update(|state| {
                let region = u64::try_from(len)
                    .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                    .and_then(|length| {
                        state
                            .core
                            .dma
                            .allocate(length)
                            .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                    });
                let changed = region.is_ok();
                (region, changed)
            })
            .await
    }

    /// Proposes DMA regions without changing mappings, bytes, or allocator generation.
    pub async fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.shared
            .backend
            .interact(|state| state.core.dma.stage(lengths))
            .await
    }

    /// Atomically commits every region in a stage.
    pub async fn commit_memory_stage(
        &self,
        proposal: PdmaMemoryStage,
    ) -> Result<Vec<MappedDmaRegion>, PdmaMemoryError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.dma.commit(proposal);
                (result, true)
            })
            .await
    }

    /// Reads one checked mapped-memory extent.
    pub async fn read_memory(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        self.shared
            .backend
            .interact(|state| state.core.dma.read_memory(address, length))
            .await
    }

    /// Writes one checked mapped-memory extent.
    pub async fn write_memory(
        &self,
        address: DmaAddress,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.dma.write_memory(address, data);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }

    /// Resolves a semantic object to its committed DMA region.
    pub async fn region_for_object(&self, object: BufferId) -> Option<MappedDmaRegion> {
        self.shared
            .backend
            .interact(|state| state.core.dma.region_for_object(object))
            .await
    }

    /// Returns the committed DMA allocator generation.
    pub async fn memory_generation(&self) -> u64 {
        self.shared
            .backend
            .interact(|state| state.core.dma.generation())
            .await
    }

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        self.allocate_memory_region(len)
            .await
            .map(crate::memory::MappedDmaRegion::object)
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.dma.write_object(id, offset, data);
                (result, true)
            })
            .await
    }

    /// Reads bytes from a checked device buffer range.
    pub async fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.shared
            .backend
            .interact(|state| state.core.dma.read_object(id, offset, len))
            .await
    }
}
