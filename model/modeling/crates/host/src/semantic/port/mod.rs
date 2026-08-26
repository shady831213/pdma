//! Async device-facing operations required by host driver turns.

mod async_device;
mod sync_device;

use crate::ModelResult;
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};

/// Async device-facing operations required by a host driver turn.
pub trait DriverPort: Send + Sync {
    /// Provisions one command queue and one bound completion ring.
    fn create_queue(
        &mut self,
        config: QueueConfig,
    ) -> impl Future<Output = ModelResult<QueueConfigHandle>> + Send;
    /// Allocates a device-visible memory buffer.
    fn allocate_buffer(&mut self, len: usize)
    -> impl Future<Output = ModelResult<BufferId>> + Send;
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
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Captures a software doorbell for one queue.
    fn ring_doorbell(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Pops one completion from a completion ring.
    fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<Option<CompletionRecord>>> + Send;
    /// Pops one event from a queue-local event path.
    fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<Option<EventRecord>>> + Send;
    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Enables a PDMA-local IRQ source.
    fn enable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Disables a PDMA-local IRQ source.
    fn disable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Masks a PDMA-local IRQ source without clearing pending state.
    fn mask_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Unmasks a PDMA-local IRQ source.
    fn unmask_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Acknowledges one source.
    fn ack_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;
    /// Returns the current pending line IRQ snapshot.
    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;
    /// Waits until the device has an asserted line IRQ snapshot.
    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;
    /// Waits for and consumes one line or message IRQ notification.
    fn wait_irq_notification(&mut self) -> impl Future<Output = Option<IrqNotification>> + Send;
}
