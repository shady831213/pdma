use super::DriverPort;
use crate::ModelResult;
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use pdma_model_device::SyncDevice;

impl DriverPort for SyncDevice {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        Self::create_queue(self, config).map_err(Into::into)
    }

    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        Self::allocate_buffer(self, len).map_err(Into::into)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        Self::write_buffer(self, id, offset, data).map_err(Into::into)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        Self::read_buffer(self, id, offset, len).map_err(Into::into)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        Self::submit_command(self, queue, command).map_err(Into::into)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        Self::ring_doorbell(self, queue).map_err(Into::into)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        Self::poll_completion(self, queue).map_err(Into::into)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        Self::poll_queue_event(self, queue).map_err(Into::into)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        Self::configure_irq_route(self, source, route).map_err(Into::into)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::enable_irq_source(self, source).map_err(Into::into)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::disable_irq_source(self, source).map_err(Into::into)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::mask_irq_source(self, source).map_err(Into::into)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::unmask_irq_source(self, source).map_err(Into::into)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::ack_irq_source(self, source).map_err(Into::into)
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        Self::irq_snapshot(self)
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        Self::wait_irq(self)
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        Self::wait_irq_notification(self)
    }
}
