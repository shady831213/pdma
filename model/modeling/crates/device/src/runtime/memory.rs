use crate::ModelResult;
use crate::runtime::{AsyncDeviceHandle, wake};
use pdma_model_contract::semantic::BufferId;

impl AsyncDeviceHandle {
    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        let (buffer, generation) = self
            .shared
            .backend
            .interact(|state| {
                let buffer = u64::try_from(len)
                    .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                    .and_then(|length| {
                        state
                            .core
                            .dma
                            .allocate(length)
                            .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                    })
                    .map(crate::dma::MappedDmaRegion::object);
                if buffer.is_ok() {
                    wake(state);
                }
                (buffer, state.generation)
            })
            .await;
        if buffer.is_ok() {
            self.shared.backend.publish(generation);
        }
        buffer
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        let (result, generation) = self
            .shared
            .backend
            .interact(|state| {
                let result = state.core.dma.write_object(id, offset, data);
                wake(state);
                (result, state.generation)
            })
            .await;
        self.shared.backend.publish(generation);
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
            .backend
            .interact(|state| state.core.dma.read_object(id, offset, len))
            .await
    }
}
