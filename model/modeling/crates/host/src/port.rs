//! Async host/device port used by actor-style driver turns.

mod async_device;
mod bus_device;
mod sync_device;

pub use bus_device::{BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter};

use crate::ModelResult;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::BufferId;
use pdma_model_contract::irq::{IrqNotification, IrqRoute, IrqSnapshot};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::semantic::EpochIrqSource;

/// Async device-facing operations required by the host driver actor.
pub trait AsyncDevicePort: Send + Sync {
    /// Provisions one command queue and one bound completion ring.
    ///
    /// # Cancel safety
    ///
    /// The queue and its ID are created only when the future returns `Ok(_)`. Dropping it while
    /// pending leaves queue state and the next queue ID unchanged.
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
    ///
    /// # Cancel safety
    ///
    /// The future must not accept or mutate the queue until it returns `Ok(())`. Dropping it while
    /// pending leaves the command unaccepted. This successful return is the acceptance commit point
    /// used by driver reservation rollback.
    fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Captures a software doorbell for one queue.
    ///
    /// # Cancel safety
    ///
    /// Runnable state and the doorbell trace commit only when the future returns `Ok(())`.
    /// Dropping it while pending leaves both unchanged.
    fn ring_doorbell(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Pops one completion from a completion ring.
    ///
    /// # Cancel safety
    ///
    /// A record is removed only when the future returns `Ok(Some(_))`. Dropping it while pending
    /// leaves the record present; `Ok(Some(_))` is the driver's completion-reclaim commit point.
    fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<Option<CompletionRecord>>> + Send;

    /// Pops one event from a queue-local event path.
    ///
    /// # Cancel safety
    ///
    /// A record is removed only when the future returns `Ok(Some(_))`. Dropping it while pending
    /// leaves the record present; `Ok(Some(_))` is the driver's event-reclaim commit point.
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

    /// Acknowledges a source, retaining a Line cause while source records remain.
    fn ack_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send;

    /// Returns the current pending IRQ snapshot, if any.
    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;

    /// Waits until the device has an asserted IRQ snapshot.
    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send;

    /// Waits for and consumes one line or message IRQ notification.
    fn wait_irq_notification(&mut self) -> impl Future<Output = Option<IrqNotification>> + Send;
}
