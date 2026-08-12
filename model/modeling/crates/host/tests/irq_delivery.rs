#![allow(missing_docs)]

use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;

use pdma_model_contract::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector, QueueConfig, QueueConfigHandle, desc::CopyRequest,
};
use pdma_model_device::SyncDevice;
use pdma_model_host::{AsyncDevicePort, AsyncDriver, SoftwareCpu};
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

    async fn allocate_buffer(&mut self, _len: usize) -> ModelResult<BufferId> {
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
        _queue: QueueConfigHandle,
        _command: ExternalCommand,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        std::future::pending().await
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        std::future::pending().await
    }

    async fn configure_irq_route(
        &mut self,
        _source: EpochIrqSource,
        _route: IrqRoute,
    ) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn enable_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn disable_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn mask_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn unmask_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
        std::future::pending().await
    }

    async fn ack_irq_source(&mut self, _source: EpochIrqSource) -> ModelResult<()> {
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

#[tokio::test(flavor = "current_thread")]
async fn stale_notification_fails_before_drain_or_ack() {
    // Given: one pending real completion and a same-source notification from a later epoch.
    let mut device = SyncDevice::new();
    let cpu = SoftwareCpu::new();
    let mut driver = cpu
        .run_main(AsyncDriver::setup_on(&mut device, 1, 1))
        .await
        .expect("active setup succeeds");
    let current = driver.queue_config_handle().completion_irq_source();
    device
        .enable_irq_source(current)
        .expect("current completion source enables");
    device
        .unmask_irq_source(current)
        .expect("current completion source unmasks");
    let (source, destination) = cpu
        .run_main(async {
            let source = driver
                .allocate_buffer_on(&mut device, 1)
                .await
                .expect("allocation");
            let destination = driver
                .allocate_buffer_on(&mut device, 1)
                .await
                .expect("allocation");
            driver.write_buffer_on(&mut device, source, &[9]).await?;
            driver
                .submit_copy_on(&mut device, CopyRequest::new(source, destination, 1))
                .await?;
            Ok::<_, ModelError>((source, destination))
        })
        .await
        .expect("current capability submits");
    device.run_until_idle(1).expect("completion publishes");
    let current_notification = device
        .wait_irq_notification()
        .expect("current completion produces a notification");
    let stale_epoch = DeviceEpoch::initial()
        .next()
        .expect("initial epoch can advance");
    let stale_source = EpochIrqSource::new(stale_epoch, current.source());
    let stale_notification = match current_notification {
        IrqNotification::Line(snapshot) => IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(snapshot.cause().line(), stale_source),
            snapshot.asserted(),
        )),
        IrqNotification::Message(message) => IrqNotification::Message(IrqMessage::new(
            message.id(),
            message.vector(),
            stale_source,
        )),
    };
    device.enable_tracing();
    let before = device.trace_events();

    // When: a wrong CPU and then the owning CPU service the stale host-owned notification.
    let wrong_cpu = SoftwareCpu::new()
        .run_main(driver.service_irq_on(&mut device, stale_notification))
        .await;
    let result = cpu
        .run_main(driver.service_irq_on(&mut device, stale_notification))
        .await;

    // Then: CPU precedence and stale epoch rejection preserve the record and accounting.
    assert_eq!(wrong_cpu, Err(ModelError::SoftwareCpuMismatch));
    assert_eq!(
        result,
        Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(device.trace_events(), before);
    assert_eq!(
        cpu.run_main(driver.submit_copy_on(&mut device, CopyRequest::new(source, destination, 1)))
            .await,
        Err(ModelError::DriverQueueFull)
    );
    let records = cpu
        .run_main(driver.service_irq_on(&mut device, current_notification))
        .await
        .expect("current notification drains and acknowledges");
    assert_eq!(records.len(), 1);
    assert!(device.irq_snapshot().is_none());
    let resumed_submit = cpu
        .run_main(driver.submit_copy_on(&mut device, CopyRequest::new(source, destination, 1)))
        .await;
    assert!(resumed_submit.is_ok());
    println!(
        "delivery epochs current={} stale={} wrong_cpu={wrong_cpu:?} stale={result:?} \
         trace_unchanged=true record_retained=true accounting_retained=true drained={} \
         ack_cleared=true resumed_submit={resumed_submit:?}",
        current.epoch().raw(),
        stale_epoch.raw(),
        records.len(),
    );
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
pub mod stale_reset;

#[tokio::test(flavor = "current_thread")]
async fn stale_notification_terminates_old_delivery_without_ack() -> ModelResult<()> {
    stale_reset::run().await
}
