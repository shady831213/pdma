#![allow(missing_docs)]

use pdma_model_contract::{
    BufferId, CompletionRecord, CompletionRingId, EventRecord, ExternalCommand, IrqCause, IrqLine,
    IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource, IrqVector,
    ModelResult, QueueConfig, QueueConfigHandle, QueueId,
};
use pdma_model_host::AsyncDevicePort;
use tokio::sync::mpsc;

#[derive(Debug)]
struct NotificationPort {
    notifications: mpsc::Receiver<IrqNotification>,
}

impl NotificationPort {
    fn channel(capacity: usize) -> (Self, mpsc::Sender<IrqNotification>) {
        let (sender, notifications) = mpsc::channel(capacity);
        (Self { notifications }, sender)
    }
}

impl AsyncDevicePort for NotificationPort {
    async fn create_queue(&mut self, _config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        std::future::pending().await
    }

    async fn allocate_buffer(&mut self, _len: usize) -> BufferId {
        std::future::pending().await
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        _len: usize,
    ) -> ModelResult<Vec<u8>> {
        std::future::pending().await
    }

    async fn submit_command(
        &mut self,
        _queue_id: QueueId,
        _command: ExternalCommand,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ring_doorbell(&mut self, _queue_id: QueueId) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn poll_completion(
        &mut self,
        _completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        std::future::pending().await
    }

    async fn poll_queue_event(&mut self, _queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        std::future::pending().await
    }

    async fn configure_irq_route(
        &mut self,
        _source: IrqSource,
        _route: IrqRoute,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn enable_irq_source(&mut self, _source: IrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn disable_irq_source(&mut self, _source: IrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn mask_irq_source(&mut self, _source: IrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn unmask_irq_source(&mut self, _source: IrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ack_irq_source(&mut self, _source: IrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        std::future::pending().await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        std::future::pending().await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.notifications.recv().await
    }
}

const fn line(vector: IrqVector, source: IrqSource) -> IrqNotification {
    IrqNotification::Line(IrqSnapshot::new(
        IrqCause::new(IrqLine::new(vector), source),
        true,
    ))
}

const fn message(id: usize, vector: IrqVector, source: IrqSource) -> IrqNotification {
    IrqNotification::Message(IrqMessage::new(IrqMessageId::new(id), vector, source))
}

#[path = "irq_delivery/atomicity.rs"]
mod atomicity;
#[path = "irq_delivery/handler_error.rs"]
mod handler_error;
#[path = "irq_delivery/registry.rs"]
mod registry;
#[path = "irq_delivery/shutdown.rs"]
mod shutdown;
