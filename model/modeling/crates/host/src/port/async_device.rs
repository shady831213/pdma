use super::AsyncDevicePort;
use pdma_model_contract::{
    BufferId, CompletionRecord, CompletionRingId, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, IrqSource, ModelResult, QueueConfig, QueueConfigHandle, QueueId,
};
use pdma_model_device::AsyncDeviceHandle;

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

    async fn configure_irq_route(&mut self, source: IrqSource, route: IrqRoute) -> ModelResult<()> {
        Self::configure_irq_route(self, source, route).await
    }

    async fn enable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        Self::enable_irq_source(self, source).await
    }

    async fn disable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        Self::disable_irq_source(self, source).await
    }

    async fn mask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        Self::mask_irq_source(self, source).await
    }

    async fn unmask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        Self::unmask_irq_source(self, source).await
    }

    async fn ack_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        Self::ack_irq_source(self, source).await
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        Self::irq_snapshot(self).await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        Self::wait_irq(self).await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        Self::wait_irq_notification(self).await
    }
}
