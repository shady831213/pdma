//! Async host/device port used by actor-style driver turns.

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_device_async::AsyncDeviceHandle;

/// Async device-facing operations required by the host driver actor.
pub trait AsyncDevicePort: Send + Sync {
    /// Provisions one command queue and one bound completion ring.
    fn create_queue(
        &mut self,
        config: QueueConfig,
    ) -> impl Future<Output = ModelResult<QueueConfigHandle>> + Send;

    /// Allocates a device-visible memory buffer.
    fn allocate_buffer(&mut self, len: usize) -> impl Future<Output = BufferId> + Send;

    /// Writes bytes into a checked device buffer range.
    fn write_buffer(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Reads bytes from a checked device buffer range.
    fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> impl Future<Output = ModelResult<Vec<u8>>> + Send;

    /// Pushes one command into a queue's command ring.
    fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Captures a software doorbell for one queue.
    fn ring_doorbell(&mut self, queue_id: QueueId) -> impl Future<Output = ModelResult<()>> + Send;

    /// Pops one completion from a completion ring.
    fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> impl Future<Output = ModelResult<Option<CompletionRecord>>> + Send;

    /// Pops one event from a queue-local event path.
    fn poll_queue_event(
        &mut self,
        queue_id: QueueId,
    ) -> impl Future<Output = ModelResult<Option<EventRecord>>> + Send;
}

impl AsyncDevicePort for AsyncDeviceHandle {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        Self::create_queue(self, config).await
    }

    async fn allocate_buffer(&mut self, len: usize) -> BufferId {
        Self::allocate_buffer(self, len).await
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        Self::write_buffer(self, id, offset, data).await
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        Self::read_buffer(self, id, offset, len).await
    }

    async fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        Self::submit_command(self, queue_id, command).await
    }

    async fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        Self::ring_doorbell(self, queue_id).await
    }

    async fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        Self::poll_completion(self, completion_ring_id).await
    }

    async fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        Self::poll_queue_event(self, queue_id).await
    }
}
