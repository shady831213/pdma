use crate::{DriverPort, PortResult};
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
    async fn create_queue(&mut self, config: QueueConfig) -> PortResult<QueueConfigHandle> {
        self.device.create_queue(config).map_err(super::port_error)
    }

    async fn allocate_buffer(&mut self, len: usize) -> PortResult<BufferId> {
        self.device.allocate_buffer(len).map_err(super::port_error)
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> PortResult<()> {
        self.device
            .write_buffer(id, offset, data)
            .map_err(super::port_error)
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> PortResult<Vec<u8>> {
        self.device
            .read_buffer(id, offset, len)
            .map_err(super::port_error)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> PortResult<()> {
        self.device
            .submit_command(queue, command)
            .map_err(super::port_error)
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> PortResult<()> {
        self.device.ring_doorbell(queue).map_err(super::port_error)
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<CompletionRecord>> {
        self.device
            .poll_completion(queue)
            .map_err(super::port_error)
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<EventRecord>> {
        self.device
            .poll_queue_event(queue)
            .map_err(super::port_error)
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> PortResult<()> {
        self.device
            .configure_irq_route(source, route)
            .map_err(super::port_error)
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.device
            .enable_irq_source(source)
            .map_err(super::port_error)
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.device
            .disable_irq_source(source)
            .map_err(super::port_error)
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.device
            .mask_irq_source(source)
            .map_err(super::port_error)
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.device
            .unmask_irq_source(source)
            .map_err(super::port_error)
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        self.device
            .ack_irq_source(source)
            .map_err(super::port_error)
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
