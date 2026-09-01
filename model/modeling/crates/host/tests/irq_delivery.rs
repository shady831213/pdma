#![allow(missing_docs)]

use crate::{DriverError, PortError, PortResult};

use crate::{AsyncDriver, DriverPort as AsyncDevicePort};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    BufferId, Command, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, IrqCause,
    IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector, QueueConfig, QueueConfigHandle,
};
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
    async fn create_queue(&mut self, _config: QueueConfig) -> PortResult<QueueConfigHandle> {
        std::future::pending().await
    }

    async fn allocate_buffer(&mut self, _len: usize) -> PortResult<BufferId> {
        std::future::pending().await
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn read_buffer(&self, _id: BufferId, _offset: usize, _len: usize) -> PortResult<Vec<u8>> {
        std::future::pending().await
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: Command,
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> PortResult<()> {
        std::future::pending().await
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> PortResult<Option<CompletionRecord>> {
        std::future::pending().await
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> PortResult<Option<EventRecord>> {
        std::future::pending().await
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> PortResult<()> {
        std::future::pending().await
    }

    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
        std::future::pending().await
    }

    async fn ack_irq_source(&mut self, _source: EpochIrqSource) -> PortResult<()> {
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
        IrqCause::new(
            IrqLine::new(vector),
            EpochIrqSource::new(DeviceEpoch::initial(), source),
        ),
        true,
    ))
}

const fn message(id: usize, vector: IrqVector, source: IrqSource) -> IrqNotification {
    IrqNotification::Message(IrqMessage::new(
        IrqMessageId::new(id),
        vector,
        EpochIrqSource::new(DeviceEpoch::initial(), source),
    ))
}

#[path = "irq_delivery/atomicity.rs"]
mod atomicity;
#[path = "irq_delivery/bootstrap.rs"]
mod bootstrap;
#[path = "irq_delivery/handler_error.rs"]
mod handler_error;
#[path = "irq_delivery/registry.rs"]
mod registry;
#[path = "irq_delivery/shutdown.rs"]
mod shutdown;
#[path = "irq_delivery/stale_reset.rs"]
pub(super) mod stale_reset;

#[tokio::test(flavor = "current_thread")]
async fn stale_notification_terminates_old_delivery_without_ack()
-> Result<(), Box<dyn std::error::Error>> {
    stale_reset::run().await
}
