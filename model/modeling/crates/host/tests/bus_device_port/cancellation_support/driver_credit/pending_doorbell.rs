use super::*;
use pdma_model_host::ModelResult;

pub(super) struct PendingDoorbellPort {
    port: BusDevicePort<pdma_model_device::AsyncDeviceHandle>,
    pending_submission: bool,
    pending_doorbell: bool,
}

impl PendingDoorbellPort {
    pub(super) const fn new(port: BusDevicePort<pdma_model_device::AsyncDeviceHandle>) -> Self {
        Self {
            port,
            pending_submission: false,
            pending_doorbell: true,
        }
    }

    pub(super) const fn with_pending_submission(
        port: BusDevicePort<pdma_model_device::AsyncDeviceHandle>,
    ) -> Self {
        Self {
            port,
            pending_submission: true,
            pending_doorbell: false,
        }
    }

    pub(super) const fn release_submission(&mut self) {
        self.pending_submission = false;
    }
}

impl AsyncDevicePort for PendingDoorbellPort {
    fn create_queue(
        &mut self,
        config: QueueConfig,
    ) -> impl Future<Output = ModelResult<QueueConfigHandle>> + Send {
        self.port.create_queue(config)
    }

    fn allocate_buffer(
        &mut self,
        len: usize,
    ) -> impl Future<Output = ModelResult<BufferId>> + Send {
        self.port.allocate_buffer(len)
    }

    fn write_buffer(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.write_buffer(id, offset, data)
    }

    fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> impl Future<Output = ModelResult<Vec<u8>>> + Send {
        self.port.read_buffer(id, offset, len)
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        if self.pending_submission {
            pending().await
        } else {
            self.port.submit_command(queue, command).await
        }
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        if self.pending_doorbell {
            pending().await
        } else {
            self.port.ring_doorbell(queue).await
        }
    }

    fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<Option<CompletionRecord>>> + Send {
        self.port.poll_completion(queue)
    }

    fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> impl Future<Output = ModelResult<Option<EventRecord>>> + Send {
        self.port.poll_queue_event(queue)
    }

    fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.configure_irq_route(source, route)
    }

    fn enable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.enable_irq_source(source)
    }

    fn disable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.disable_irq_source(source)
    }

    fn mask_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.mask_irq_source(source)
    }

    fn unmask_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.unmask_irq_source(source)
    }

    fn ack_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> impl Future<Output = ModelResult<()>> + Send {
        self.port.ack_irq_source(source)
    }

    fn irq_snapshot(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        self.port.irq_snapshot()
    }

    fn wait_irq(&self) -> impl Future<Output = Option<IrqSnapshot>> + Send {
        self.port.wait_irq()
    }

    fn wait_irq_notification(&mut self) -> impl Future<Output = Option<IrqNotification>> + Send {
        self.port.wait_irq_notification()
    }
}
use crate::register::legacy_integration_tests::pdma_model_host;
