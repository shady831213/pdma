//! Async queue capability surface.

use crate::ModelResult;
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EventRecord, QueueConfig, QueueConfigHandle,
};

impl AsyncDeviceHandle {
    /// Provisions one command queue and one bound completion ring.
    pub async fn create_queue(&self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.with_device(|device| device.create_queue(config)).await
    }

    /// Pushes one command through an epoch-qualified queue capability.
    pub async fn submit_command(
        &self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> ModelResult<()> {
        self.with_device(|device| device.submit_command(queue, command))
            .await
    }

    /// Captures a software doorbell through an epoch-qualified queue capability.
    pub async fn ring_doorbell(&self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.with_device(|device| device.ring_doorbell(queue)).await
    }

    /// Pops one completion through its epoch-qualified queue capability.
    pub async fn poll_completion(
        &self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.with_device(|device| device.poll_completion(queue))
            .await
    }

    /// Pops one event through its epoch-qualified queue capability.
    pub async fn poll_queue_event(
        &self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.with_device(|device| device.poll_queue_event(queue))
            .await
    }
}
