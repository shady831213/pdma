use crate::{DriverPort, ModelResult};
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqNotification, IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle, WholeDeviceReset,
};
use pdma_model_device::{PdmaResetError, compat::SyncDevice};

/// Borrowed semantic compatibility port for deterministic manual devices.
#[derive(Debug)]
pub struct SemanticDevicePort<'device> {
    device: &'device mut SyncDevice,
}

impl<'device> SemanticDevicePort<'device> {
    /// Borrows a synchronous device until this adapter is dropped.
    pub const fn new(device: &'device mut SyncDevice) -> Self {
        Self { device }
    }

    /// Resets through the shared semantic reset seam.
    pub fn reset(&mut self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaResetError> {
        self.device.reset(request)
    }
}

impl DriverPort for SemanticDevicePort<'_> {
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.device.create_queue(config).map_err(Into::into)
    }

    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        self.device.allocate_buffer(len).map_err(Into::into)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.device
            .write_buffer(id, offset, data)
            .map_err(Into::into)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.device.read_buffer(id, offset, len).map_err(Into::into)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.device
            .submit_command(queue, command)
            .map_err(Into::into)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.device.ring_doorbell(queue).map_err(Into::into)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.device.poll_completion(queue).map_err(Into::into)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.device.poll_queue_event(queue).map_err(Into::into)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.device
            .configure_irq_route(source, route)
            .map_err(Into::into)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.device.enable_irq_source(source).map_err(Into::into)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.device.disable_irq_source(source).map_err(Into::into)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.device.mask_irq_source(source).map_err(Into::into)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.device.unmask_irq_source(source).map_err(Into::into)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.device.ack_irq_source(source).map_err(Into::into)
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.device.irq_snapshot()
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.device.wait_irq()
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.device.wait_irq_notification()
    }
}
