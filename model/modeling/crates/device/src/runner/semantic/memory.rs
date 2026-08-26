use crate::ModelResult;
use crate::dma_space::{PdmaMemoryError, PdmaMemoryStage};
use crate::runner::{AsyncDeviceHandle, publish, wake};
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::binding::PdmaMemoryRegion;
use pdma_model_contract::semantic::BufferId;
use std::sync::Arc;

impl AsyncDeviceHandle {
    /// Returns the clone-shared runtime memory capability owned by this device core.
    pub fn memory_access(&self) -> Arc<hardware_interface_runtime::SharedMemory> {
        Arc::clone(&self.memory)
    }

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        self.allocate_memory_region(len)
            .await
            .map(PdmaMemoryRegion::object)
    }

    /// Allocates one raw device-visible DMA region.
    pub async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        let (region, generation) = {
            let mut state = self.shared.state.lock().await;
            let region = u64::try_from(len)
                .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                .and_then(|length| {
                    state
                        .dma_space
                        .allocate(length)
                        .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                });
            if region.is_ok() {
                wake(&mut state);
            }
            (region, state.generation)
        };
        if region.is_ok() {
            publish(&self.shared, generation);
        }
        region
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.write_object(id, offset, data);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads bytes from a checked device buffer range.
    pub async fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .read_object(id, offset, len)
    }

    /// Proposes DMA regions without changing mappings, bytes, or allocator generation.
    pub async fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.shared.state.lock().await.dma_space.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub async fn commit_memory_stage(
        &self,
        proposal: PdmaMemoryStage,
    ) -> Result<Vec<PdmaMemoryRegion>, PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.commit(proposal);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads one checked mapped-memory extent.
    pub async fn read_memory(
        &self,
        address: impl Into<MemoryAddress>,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .read_memory(address.into(), length)
    }

    /// Writes one checked mapped-memory extent.
    pub async fn write_memory(
        &self,
        address: impl Into<MemoryAddress>,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.write_memory(address.into(), data);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Resolves a semantic object to its committed DMA region.
    pub async fn region_for_object(&self, object: BufferId) -> Option<PdmaMemoryRegion> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub async fn memory_generation(&self) -> u64 {
        self.shared.state.lock().await.dma_space.generation()
    }
}
