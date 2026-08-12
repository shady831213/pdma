use pdma_model_contract::{BufferId, CommandId, CopyRequest, IrqNotification, IrqRoute, IrqVector};
use pdma_model_contract::{EpochQueue, RingCursor};
use pdma_model_device::AsyncDevice;
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use pdma_model_host::{
    AsyncDevicePort, AsyncDriver, AsyncPdmaBus, BusDevicePort, DriverIrqRecord, IrqDelivery,
    QueueControlWrite, SoftwareCpu, StatefulIrqHandler, stateful_irq_handler,
};
use tokio::sync::oneshot;

#[path = "consumer_race_support/gated_bus.rs"]
mod gated_bus;

use gated_bus::GatedBus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Host-consumed publication lane selected for one fresh race fixture.
pub enum ConsumerLane {
    /// Completion-ring consumer publication.
    Completion,
    /// Event-ring consumer publication.
    Event,
}

impl ConsumerLane {
    const fn name(self) -> &'static str {
        match self {
            Self::Completion => "completion",
            Self::Event => "event",
        }
    }

    const fn source(self, driver: &AsyncDriver) -> pdma_model_contract::EpochIrqSource {
        match self {
            Self::Completion => driver.completion_irq_source(),
            Self::Event => driver.event_irq_source(),
        }
    }

    const fn cursor(self, status: pdma_model_host::BusQueueStatus) -> RingCursor {
        match self {
            Self::Completion => status.completion().consumer(),
            Self::Event => status.event().consumer(),
        }
    }

    pub(super) const fn matches(self, request: QueueControlWrite) -> bool {
        matches!(
            (self, request),
            (
                Self::Completion,
                QueueControlWrite::PublishCompletionConsumer(_)
            ) | (Self::Event, QueueControlWrite::PublishEventConsumer(_))
        )
    }
}

struct DrainHandler {
    driver: AsyncDriver,
    port: BusDevicePort<GatedBus>,
    report: Option<oneshot::Sender<ModelResult<Vec<DriverIrqRecord>>>>,
}

impl StatefulIrqHandler for DrainHandler {
    async fn handle(&mut self, notification: IrqNotification) -> ModelResult<()> {
        let result = self
            .driver
            .service_irq_on(&mut self.port, notification)
            .await;
        self.report
            .take()
            .ok_or(ModelError::DmaSubmissionState)?
            .send(result)
            .map_err(|_| ModelError::DmaSubmissionState)
    }
}

fn record_command(lane: ConsumerLane, records: &[DriverIrqRecord]) -> Option<CommandId> {
    match (lane, records) {
        (ConsumerLane::Completion, [DriverIrqRecord::Completion(record)]) => {
            Some(record.command_id())
        }
        (ConsumerLane::Event, [DriverIrqRecord::Event(record)]) => Some(record.command_id()),
        (ConsumerLane::Completion | ConsumerLane::Event, _) => None,
    }
}

async fn raw_cursor(
    handle: &pdma_model_device::AsyncDeviceHandle,
    queue: EpochQueue,
    lane: ConsumerLane,
) -> ModelResult<RingCursor> {
    AsyncPdmaBus::read_queue_status(handle, queue)
        .await
        .map(|status| lane.cursor(status))
        .map_err(|_| ModelError::DmaSubmissionState)
}

async fn poll_main(
    lane: ConsumerLane,
    driver: &mut AsyncDriver,
    port: &mut BusDevicePort<GatedBus>,
) -> ModelResult<()> {
    match lane {
        ConsumerLane::Completion => driver.poll_completion_on(port).await.map(|_| ()),
        ConsumerLane::Event => driver.poll_event_on(port).await.map(|_| ()),
    }
}

struct PortClones {
    main_port: BusDevicePort<GatedBus>,
    main_driver: AsyncDriver,
    handler_port: BusDevicePort<GatedBus>,
    handler_driver: AsyncDriver,
    probe_port: BusDevicePort<GatedBus>,
    probe_driver: AsyncDriver,
    source: BufferId,
    destination: BufferId,
}

async fn setup_ports(
    bus: &GatedBus,
    cpu: &SoftwareCpu,
    lane: ConsumerLane,
    vector: IrqVector,
) -> ModelResult<PortClones> {
    cpu.run_main(async {
        let mut port = BusDevicePort::new(bus.clone())?;
        let mut driver = AsyncDriver::setup_on(&mut port, 1, 2).await?;
        let source = driver.allocate_buffer_on(&mut port, 4).await?;
        let destination = driver.allocate_buffer_on(&mut port, 4).await?;
        driver
            .write_buffer_on(&mut port, source, &[1, 2, 3, 4])
            .await?;
        let irq = lane.source(&driver);
        port.configure_irq_route(irq, IrqRoute::Message(vector))
            .await?;
        port.enable_irq_source(irq).await?;
        port.unmask_irq_source(irq).await?;
        let len = match lane {
            ConsumerLane::Completion => 4,
            ConsumerLane::Event => 8,
        };
        let command = driver
            .submit_copy_on(&mut port, CopyRequest::new(source, destination, len))
            .await?;
        assert_eq!(command, CommandId::new(0));
        Ok(PortClones {
            main_port: port.clone(),
            main_driver: driver.clone(),
            handler_port: port.clone(),
            handler_driver: driver.clone(),
            probe_port: port,
            probe_driver: driver,
            source,
            destination,
        })
    })
    .await
}

fn print_observation(lane: ConsumerLane) {
    println!(
        "lane={} blocked_cursor=0 handler_cursor=1 final_cursor=1 handler_records=1 handler_command=0 credit_before=full credit_after_handler=command1 final_credit=full matching_polls=2 delegated_writes=1 main=stale",
        lane.name()
    );
}

/// Runs one deterministic clone-shared consumer race through the real actors.
///
/// # Panics
/// Panics when an intermediate or final test observation violates the race contract.
pub async fn run_consumer_race(lane: ConsumerLane) -> ModelResult<()> {
    let (device, worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let worker_task = tokio::spawn(worker.run());
    let (bus, notification_sender) = GatedBus::new(handle.clone(), lane);
    let cpu = SoftwareCpu::new();
    let vector = IrqVector::new(match lane {
        ConsumerLane::Completion => 93,
        ConsumerLane::Event => 94,
    });
    let PortClones {
        mut main_port,
        mut main_driver,
        handler_port,
        handler_driver,
        mut probe_port,
        mut probe_driver,
        source,
        destination,
    } = setup_ports(&bus, &cpu, lane, vector).await?;
    let notification = handle
        .wait_irq_notification()
        .await
        .ok_or(ModelError::IrqDeliveryUnavailable)?;
    assert_eq!(notification.source_capability(), lane.source(&main_driver));
    let queue = EpochQueue::new(
        main_driver.queue_config_handle().epoch(),
        main_driver.queue_id(),
    );
    assert_eq!(raw_cursor(&handle, queue, lane).await?, RingCursor::new(0));
    let credit_before = cpu
        .run_main(
            probe_driver.submit_copy_on(&mut probe_port, CopyRequest::new(source, destination, 4)),
        )
        .await;
    assert_eq!(credit_before, Err(ModelError::DriverQueueFull));

    let (report_sender, report_receiver) = oneshot::channel();
    let (delivery, delivery_handle) = IrqDelivery::new(handler_port.clone(), cpu.clone())
        .register_handler(
            vector,
            stateful_irq_handler(DrainHandler {
                driver: handler_driver,
                port: handler_port,
                report: Some(report_sender),
            }),
        );
    let delivery_task = tokio::spawn(delivery.run());
    bus.arm();
    let main_cpu = cpu.clone();
    let main_task = tokio::spawn(async move {
        main_cpu
            .run_main(poll_main(lane, &mut main_driver, &mut main_port))
            .await
    });
    bus.wait_entered().await;
    let blocked_cursor = raw_cursor(&handle, queue, lane).await?;
    assert_eq!((blocked_cursor, bus.counts()), (RingCursor::new(0), (1, 0)));

    notification_sender
        .send(notification)
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let records = report_receiver
        .await
        .map_err(|_| ModelError::DmaSubmissionState)??;
    assert_eq!(record_command(lane, &records), Some(CommandId::new(0)));
    let handler_cursor = raw_cursor(&handle, queue, lane).await?;
    assert_eq!((handler_cursor, bus.counts()), (RingCursor::new(1), (2, 1)));
    let command_one = cpu
        .run_main(
            probe_driver.submit_copy_on(&mut probe_port, CopyRequest::new(source, destination, 4)),
        )
        .await?;
    assert_eq!(command_one, CommandId::new(1));

    bus.release();
    let main_result = main_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    assert_eq!(main_result, Err(ModelError::DmaSubmissionState));
    let final_cursor = raw_cursor(&handle, queue, lane).await?;
    let command_two = cpu
        .run_main(
            probe_driver.submit_copy_on(&mut probe_port, CopyRequest::new(source, destination, 4)),
        )
        .await;
    assert_eq!(command_two, Err(ModelError::DriverQueueFull));
    assert_eq!((final_cursor, bus.counts()), (RingCursor::new(1), (2, 1)));

    delivery_handle.request_shutdown();
    let delivery_summary = delivery_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)??;
    assert_eq!(delivery_summary.deliveries(), 1);
    device.request_shutdown().await;
    worker_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    print_observation(lane);
    Ok(())
}
