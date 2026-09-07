use crate::ModelResult;
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::BufferId;

impl AsyncDeviceHandle {
    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        self.allocate_memory_region(len)
            .await
            .map(crate::dma::MappedDmaRegion::object)
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
