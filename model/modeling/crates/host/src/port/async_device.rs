use super::AsyncDevicePort;
use crate::ModelResult;
use pdma_model_contract::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use pdma_model_device::AsyncDeviceHandle;

impl AsyncDevicePort for AsyncDeviceHandle {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        Self::create_queue(self, config).await.map_err(Into::into)
    }

    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        Self::allocate_buffer(self, len).await.map_err(Into::into)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        Self::write_buffer(self, id, offset, data)
            .await
            .map_err(Into::into)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        Self::read_buffer(self, id, offset, len)
            .await
            .map_err(Into::into)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        Self::submit_command(self, queue, command)
            .await
            .map_err(Into::into)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        Self::ring_doorbell(self, queue).await.map_err(Into::into)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        Self::poll_completion(self, queue).await.map_err(Into::into)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        Self::poll_queue_event(self, queue)
            .await
            .map_err(Into::into)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        Self::configure_irq_route(self, source, route)
            .await
            .map_err(Into::into)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::enable_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::disable_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::mask_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::unmask_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        Self::ack_irq_source(self, source).await.map_err(Into::into)
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
