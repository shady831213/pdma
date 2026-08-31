use crate::ModelResult;
use crate::runner::{AsyncDeviceHandle, publish, wake};
use pdma_model_contract::semantic::BufferId;

impl AsyncDeviceHandle {
    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        let (buffer, generation) = {
            let mut state = self.shared.state.lock().await;
            let buffer = u64::try_from(len)
                .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                .and_then(|length| {
                    state
                        .dma_space
                        .allocate(length)
                        .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                })
                .map(crate::dma_space::MappedDmaRegion::object);
            if buffer.is_ok() {
                wake(&mut state);
            }
            (buffer, state.generation)
        };
        if buffer.is_ok() {
            publish(&self.shared, generation);
        }
        buffer
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
}
