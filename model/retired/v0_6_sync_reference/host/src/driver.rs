//! SDK/driver-like submission and completion polling model.

mod actor;

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::{
    BroadcastCopyRequest, CopyRequest, StridedCopyRequest, VectorAddRequest,
};
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CommandId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_device::PdmaDevice;

/// Software driver model owning queue setup and local ring-space accounting.
#[derive(Debug, Clone)]
pub struct Driver {
    queue_config: QueueConfigHandle,
    next_command: usize,
    free_command_slots: usize,
}

impl Driver {
    /// Sets up one command queue and bound completion ring on the device.
    pub fn setup(
        device: &mut PdmaDevice,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<Self> {
        Self::setup_on(device, command_depth, completion_depth)
    }

    /// Sets up one command queue from an explicit queue configuration.
    pub fn setup_with_config(device: &mut PdmaDevice, config: QueueConfig) -> ModelResult<Self> {
        Self::setup_with_config_on(device, config)
    }

    /// Allocates a device-visible buffer.
    pub fn allocate_buffer(&self, device: &mut PdmaDevice, len: usize) -> BufferId {
        self.allocate_buffer_on(device, len)
    }

    /// Writes a complete buffer payload at offset zero.
    pub fn write_buffer(
        &self,
        device: &mut PdmaDevice,
        buffer: BufferId,
        data: &[u8],
    ) -> ModelResult<()> {
        self.write_buffer_on(device, buffer, data)
    }

    /// Reads a checked buffer payload at offset zero.
    pub fn read_buffer(
        &self,
        device: &PdmaDevice,
        buffer: BufferId,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.read_buffer_on(device, buffer, len)
    }

    /// Emits a copy descriptor to the command ring and rings the doorbell.
    pub fn submit_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: CopyRequest,
    ) -> ModelResult<CommandId> {
        self.submit_copy_on(device, request)
    }

    /// Emits a strided copy descriptor to the command ring and rings the doorbell.
    pub fn submit_strided_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: StridedCopyRequest,
    ) -> ModelResult<CommandId> {
        self.submit_strided_copy_on(device, request)
    }

    /// Emits a broadcast copy descriptor to the command ring and rings the doorbell.
    pub fn submit_broadcast_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: BroadcastCopyRequest,
    ) -> ModelResult<CommandId> {
        self.submit_broadcast_copy_on(device, request)
    }

    /// Emits a vector-add descriptor to the command ring and rings the doorbell.
    pub fn submit_vector_add(
        &mut self,
        device: &mut PdmaDevice,
        request: VectorAddRequest,
    ) -> ModelResult<CommandId> {
        self.submit_vector_add_on(device, request)
    }

    /// Polls one completion and reclaims one local command-ring slot if present.
    pub fn poll_completion(
        &mut self,
        device: &mut PdmaDevice,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.poll_completion_on(device)
    }

    /// Polls one event and reclaims one local command-ring slot if present.
    pub fn poll_event(&mut self, device: &mut PdmaDevice) -> ModelResult<Option<EventRecord>> {
        self.poll_event_on(device)
    }

    /// Queue ID owned by this driver instance.
    pub const fn queue_id(&self) -> QueueId {
        self.queue_config.queue_id()
    }

    /// Completion ring ID owned by this driver instance.
    pub const fn completion_ring_id(&self) -> CompletionRingId {
        self.queue_config.completion_ring_id()
    }

    /// Queue configuration handle owned by this driver instance.
    pub const fn queue_config_handle(&self) -> QueueConfigHandle {
        self.queue_config
    }
}

#[cfg(test)]
mod tests {
    use super::Driver;
    use pdma_model_contract::desc::CopyRequest;
    use pdma_model_contract::error::ModelError;
    use pdma_model_contract::ids::CommandId;
    use pdma_model_device::device::PdmaDevice;

    #[test]
    fn submit_copy_writes_command_and_uses_local_shadow_slots() {
        // Given: a driver with one command slot.
        let mut device = PdmaDevice::new();
        let mut driver = Driver::setup(&mut device, 1, 1).expect("queue setup is valid");
        let src = driver.allocate_buffer(&mut device, 4);
        let dst = driver.allocate_buffer(&mut device, 4);

        // When: one copy is submitted.
        let command_id = driver
            .submit_copy(&mut device, CopyRequest::new(src, dst, 4))
            .expect("first command has a free slot");

        // Then: the command ID is assigned and a second submit is locally rejected.
        assert_eq!(command_id, CommandId::new(0));
        assert_eq!(
            driver.submit_copy(&mut device, CopyRequest::new(src, dst, 4)),
            Err(ModelError::DriverQueueFull)
        );
    }
}
