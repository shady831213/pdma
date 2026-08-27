#![allow(missing_docs)]

use super::pdma_model_host;
use pdma_model_host::{DriverError, PortError, PortResult};

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand,
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector, QueueConfig, QueueConfigHandle, desc::CopyRequest,
};
use pdma_model_device::SyncDevice;
use pdma_model_host::{AsyncDriver, DriverPort as AsyncDevicePort, compat::SemanticDevicePort};
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

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        _len: usize,
    ) -> PortResult<Vec<u8>> {
        std::future::pending().await
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        _command: ExternalCommand,
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

#[tokio::test(flavor = "current_thread")]
async fn stale_notification_fails_before_drain_or_ack() {
    // Given: one pending real completion and a same-source notification from a later epoch.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let cpu = SoftwareCpu::new();
    let mut port = SemanticDevicePort::new(&mut device);
    let mut driver = cpu
        .run_main(AsyncDriver::setup_on(&mut port, 1, 1))
        .await
        .expect("active setup succeeds");
    let current = driver.queue_config_handle().completion_irq_source();
    drop(port);
    device
        .enable_irq_source(current)
        .expect("current completion source enables");
    device
        .unmask_irq_source(current)
        .expect("current completion source unmasks");
    let (source, destination) = cpu
        .run_main(async {
            let mut port = SemanticDevicePort::new(&mut device);
            let source = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("allocation");
            let destination = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("allocation");
            driver.write_buffer_on(&mut port, source, &[9]).await?;
            driver
                .submit_copy_on(&mut port, CopyRequest::new(source, destination, 1))
                .await?;
            Ok::<_, DriverError>((source, destination))
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
    let wrong_cpu = {
        let mut port = SemanticDevicePort::new(&mut device);
        SoftwareCpu::new()
            .run_main(driver.service_irq_on(&mut port, stale_notification))
            .await
    };
    let result = {
        let mut port = SemanticDevicePort::new(&mut device);
        cpu.run_main(driver.service_irq_on(&mut port, stale_notification))
            .await
    };

    // Then: CPU precedence and stale epoch rejection preserve the record and accounting.
    assert_eq!(
        wrong_cpu,
        Err(DriverError::WrongCpu(
            hardware_interface_runtime::CpuContextError::Mismatch,
        ))
    );
    assert_eq!(
        result,
        Err(DriverError::Port(PortError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        }))
    );
    assert_eq!(device.trace_events(), before);
    let blocked_submit = {
        let mut port = SemanticDevicePort::new(&mut device);
        cpu.run_main(driver.submit_copy_on(&mut port, CopyRequest::new(source, destination, 1)))
            .await
    };
    assert_eq!(blocked_submit, Err(DriverError::QueueFull));
    let records = {
        let mut port = SemanticDevicePort::new(&mut device);
        cpu.run_main(driver.service_irq_on(&mut port, current_notification))
            .await
            .expect("current notification drains and acknowledges")
    };
    assert_eq!(records.len(), 1);
    assert!(device.irq_snapshot().is_none());
    let resumed_submit = {
        let mut port = SemanticDevicePort::new(&mut device);
        cpu.run_main(driver.submit_copy_on(&mut port, CopyRequest::new(source, destination, 1)))
            .await
    };
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
pub(super) mod stale_reset;

#[tokio::test(flavor = "current_thread")]
async fn stale_notification_terminates_old_delivery_without_ack() -> Result<(), Box<dyn std::error::Error>> {
    stale_reset::run().await
}
