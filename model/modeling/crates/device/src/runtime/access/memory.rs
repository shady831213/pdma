use crate::ModelResult;
use crate::memory::{MappedDmaRegion, PdmaMemoryError};
use crate::runtime::AsyncDeviceHandle;
use crate::runtime::PdmaMemoryStage;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::BufferId;

impl AsyncDeviceHandle {
    /// Returns the clone-shared runtime memory capability owned by this device core.
    pub fn memory_access(&self) -> hardware_interface_runtime::SharedMemoryAccess {
        self.memory.clone()
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
                            .provisioning
                            .allocate(&mut state.core.memory, length)
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
            .interact(|state| state.provisioning.stage(&state.core.memory, lengths))
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
                let result = state.provisioning.commit(&mut state.core.memory, proposal);
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
            .interact(|state| state.core.memory.read_memory(address, length))
            .await
    }

    /// Registers externally provisioned addresses without allocating or overwriting bytes.
    pub async fn register_memory_regions(
        &self,
        regions: &[MappedDmaRegion],
    ) -> Result<(), PdmaMemoryError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.memory.register_regions(regions);
                let changed = result.is_ok();
                (result, changed)
            })
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
                let result = state.core.memory.write_memory(address, data);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }

    /// Resolves a semantic object to its committed DMA region.
    pub async fn region_for_object(&self, object: BufferId) -> Option<MappedDmaRegion> {
        self.shared
            .backend
            .interact(|state| state.core.memory.region_for_object(object))
            .await
    }

    /// Returns the committed DMA allocator generation.
    pub async fn memory_generation(&self) -> u64 {
        self.shared
            .backend
            .interact(|state| state.core.memory.generation())
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
                let result = state.core.memory.write_object(id, offset, data);
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
            .interact(|state| state.core.memory.read_object(id, offset, len))
            .await
    }
}
