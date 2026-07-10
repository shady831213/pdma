//! Async host/device port used by actor-style driver turns.

mod async_device;
mod sync_device;

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::irq::{IrqNotification, IrqRoute, IrqSnapshot, IrqSource};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};

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

    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    fn configure_irq_route(
        &mut self,
        source: IrqSource,
        route: IrqRoute,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Enables a PDMA-local IRQ source.
    fn enable_irq_source(
        &mut self,
        source: IrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Disables a PDMA-local IRQ source.
    fn disable_irq_source(
        &mut self,
        source: IrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Masks a PDMA-local IRQ source without clearing pending state.
    fn mask_irq_source(
        &mut self,
        source: IrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Unmasks a PDMA-local IRQ source.
    fn unmask_irq_source(
        &mut self,
        source: IrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Acknowledges and clears a PDMA-local IRQ source.
    fn ack_irq_source(&mut self, source: IrqSource)
    -> impl Future<Output = ModelResult<()>> + Send;

    /// Returns the current pending IRQ snapshot, if any.
    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;

    /// Waits until the device has an asserted IRQ snapshot.
    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;

    /// Waits for and consumes one line or message IRQ notification.
    fn wait_irq_notification(&mut self) -> impl Future<Output = Option<IrqNotification>> + Send;
}
