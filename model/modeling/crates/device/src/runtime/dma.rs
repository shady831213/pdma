use crate::ModelResult;
use crate::dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryStage};
use crate::runtime::{AsyncDeviceHandle, publish, wake};
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
        let (region, generation) = {
            let mut state = self.shared.state.lock().await;
            let region = u64::try_from(len)
                .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                .and_then(|length| {
                    state
                        .dma
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

    /// Proposes DMA regions without changing mappings, bytes, or allocator generation.
    pub async fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.shared.state.lock().await.dma.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub async fn commit_memory_stage(
        &self,
        proposal: PdmaMemoryStage,
    ) -> Result<Vec<MappedDmaRegion>, PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma.commit(proposal);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads one checked mapped-memory extent.
    pub async fn read_memory(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        self.shared
            .state
            .lock()
            .await
            .dma
            .read_memory(address, length)
    }

    /// Writes one checked mapped-memory extent.
    pub async fn write_memory(
        &self,
        address: DmaAddress,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma.write_memory(address, data);
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
    pub async fn region_for_object(&self, object: BufferId) -> Option<MappedDmaRegion> {
        self.shared.state.lock().await.dma.region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub async fn memory_generation(&self) -> u64 {
        self.shared.state.lock().await.dma.generation()
    }
}
