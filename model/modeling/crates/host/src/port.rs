//! Synchronous host/device port used by actor-style driver turns.

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_device::PdmaDevice;

/// Synchronous device-facing operations required by the host driver actor.
///
/// The trait is intentionally blocking and turn-based: callers decide when
/// software acts through this port and when hardware advances through the
/// device surface. It adds no runtime or async dependency.
pub trait DevicePort {
    /// Provisions one command queue and one bound completion ring.
    fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle>;

    /// Allocates a device-visible memory buffer.
    fn allocate_buffer(&mut self, len: usize) -> BufferId;

    /// Writes bytes into a checked device buffer range.
    fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()>;

    /// Reads bytes from a checked device buffer range.
    fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>>;

    /// Pushes one command into a queue's command ring.
    fn submit_command(&mut self, queue_id: QueueId, command: ExternalCommand) -> ModelResult<()>;

    /// Captures a software doorbell for one queue.
    fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()>;

    /// Pops one completion from a completion ring.
    fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>>;

    /// Pops one event for a specific queue without consuming other queues' events.
    fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>>;
}

impl DevicePort for PdmaDevice {
    fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        Self::create_queue(self, config)
    }

    fn allocate_buffer(&mut self, len: usize) -> BufferId {
        Self::allocate_buffer(self, len)
    }

    fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        Self::write_buffer(self, id, offset, data)
    }

    fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        Self::read_buffer(self, id, offset, len)
    }

    fn submit_command(&mut self, queue_id: QueueId, command: ExternalCommand) -> ModelResult<()> {
        Self::submit_command(self, queue_id, command)
    }

    fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        Self::ring_doorbell(self, queue_id)
    }

    fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        Self::poll_completion(self, completion_ring_id)
    }

    fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        Self::poll_queue_event(self, queue_id)
    }
}
