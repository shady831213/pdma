use crate::{DriverPort, ModelResult};
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqNotification, IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle, WholeDeviceReset,
};
use pdma_model_device::{AsyncDeviceHandle, BusControlError};

/// Explicit semantic compatibility port for an asynchronous device handle.
#[derive(Debug, Clone)]
pub struct AsyncDevicePort {
    handle: AsyncDeviceHandle,
}

impl AsyncDevicePort {
    /// Wraps one clone-shared semantic device handle.
    pub const fn new(handle: AsyncDeviceHandle) -> Self {
        Self { handle }
    }

    /// Resets through the semantic device seam without exposing register access.
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, BusControlError> {
        self.handle.reset(request).await
    }
}

impl DriverPort for AsyncDevicePort {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.handle.create_queue(config).await.map_err(Into::into)
    }

    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        self.handle.allocate_buffer(len).await.map_err(Into::into)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.handle
            .write_buffer(id, offset, data)
            .await
            .map_err(Into::into)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.handle
            .read_buffer(id, offset, len)
            .await
            .map_err(Into::into)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.handle
            .submit_command(queue, command)
            .await
            .map_err(Into::into)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.handle.ring_doorbell(queue).await.map_err(Into::into)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.handle.poll_completion(queue).await.map_err(Into::into)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.handle
            .poll_queue_event(queue)
            .await
            .map_err(Into::into)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.handle
            .configure_irq_route(source, route)
            .await
            .map_err(Into::into)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.handle
            .enable_irq_source(source)
            .await
            .map_err(Into::into)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.handle
            .disable_irq_source(source)
            .await
            .map_err(Into::into)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.handle
            .mask_irq_source(source)
            .await
            .map_err(Into::into)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.handle
            .unmask_irq_source(source)
            .await
            .map_err(Into::into)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.handle.ack_irq_source(source).await.map_err(Into::into)
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.handle.irq_snapshot().await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.handle.wait_irq().await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.handle.wait_irq_notification().await
    }
}
