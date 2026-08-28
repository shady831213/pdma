use crate::{DriverPort, PortResult};
use pdma_model_contract::semantic::{
    BufferId, Command, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle, WholeDeviceReset,
};
use pdma_model_device::{AsyncDeviceHandle, PdmaControlError};

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
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaControlError> {
        self.handle.reset(request).await
    }
}

impl DriverPort for AsyncDevicePort {
    async fn create_queue(&mut self, config: QueueConfig) -> PortResult<QueueConfigHandle> {
        self.handle
            .create_queue(config)
            .await
            .map_err(super::port_error)
    }

    async fn allocate_buffer(&mut self, len: usize) -> PortResult<BufferId> {
        self.handle
            .allocate_buffer(len)
            .await
            .map_err(super::port_error)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> PortResult<()> {
        self.handle
            .write_buffer(id, offset, data)
            .await
            .map_err(super::port_error)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> PortResult<Vec<u8>> {
        self.handle
            .read_buffer(id, offset, len)
            .await
            .map_err(super::port_error)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> PortResult<()> {
        self.handle
            .submit_command(queue, command)
            .await
            .map_err(super::port_error)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> PortResult<()> {
        self.handle
            .ring_doorbell(queue)
            .await
            .map_err(super::port_error)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<CompletionRecord>> {
        self.handle
            .poll_completion(queue)
            .await
            .map_err(super::port_error)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<EventRecord>> {
        self.handle
            .poll_queue_event(queue)
            .await
            .map_err(super::port_error)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> PortResult<()> {
        self.handle
            .configure_irq_route(source, route)
            .await
            .map_err(super::port_error)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.handle
            .enable_irq_source(source)
            .await
            .map_err(super::port_error)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.handle
            .disable_irq_source(source)
            .await
            .map_err(super::port_error)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.handle
            .mask_irq_source(source)
            .await
            .map_err(super::port_error)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.handle
            .unmask_irq_source(source)
            .await
            .map_err(super::port_error)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.handle
            .ack_irq_source(source)
            .await
            .map_err(super::port_error)
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
