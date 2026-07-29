#![allow(missing_docs)]

use pdma_model_contract::{
    CommandId, CompletionStatus, CopyRequest, EventErrorKind, IrqNotification, IrqRoute, IrqSource,
    IrqVector, ModelError, ModelResult,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};
use pdma_model_host::{
    AsyncDriver, DriverIrqRecord, IrqDelivery, SoftwareCpu, StatefulIrqHandler,
    stateful_irq_handler,
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandlerIdentity {
    Completion,
    Event,
}

#[derive(Debug)]
struct HandlerObservation {
    identity: HandlerIdentity,
    call: usize,
    notification: IrqNotification,
    records: Vec<DriverIrqRecord>,
}

struct DriverHandler {
    identity: HandlerIdentity,
    calls: usize,
    driver: AsyncDriver,
    port: AsyncDeviceHandle,
    source: IrqSource,
    report: mpsc::Sender<HandlerObservation>,
}

impl StatefulIrqHandler for DriverHandler {
    async fn handle(&mut self, notification: IrqNotification) -> ModelResult<()> {
        if notification_source(notification) != self.source {
            return Err(ModelError::UnknownIrqSource(notification_source(
                notification,
            )));
        }
        self.calls = self.calls.saturating_add(1);
        let records = self
            .driver
            .service_irq_on(&mut self.port, notification)
            .await?;
        // This report channel only makes the handler order and drained records test-visible.
        let _ = self
            .report
            .send(HandlerObservation {
                identity: self.identity,
                call: self.calls,
                notification,
                records,
            })
            .await;
        Ok(())
    }
}

struct MainOutcome {
    commands: [CommandId; 3],
    sources: [IrqSource; 2],
    observations: Vec<HandlerObservation>,
    copied: Vec<u8>,
}

#[tokio::test(flavor = "current_thread")]
async fn message_irq_two_handlers_share_driver_accounting() -> ModelResult<()> {
    // Given: two vectors dispatch to independent persistent handlers on one modeled CPU.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    let cpu = SoftwareCpu::new();
    let completion_vector = IrqVector::new(65);
    let event_vector = IrqVector::new(66);
    // Test-only rendezvous: main waits for each handler turn before its next submission.
    let (report_tx, mut report_rx) = mpsc::channel(3);
    let (delivery, delivery_runner) = IrqDelivery::new(port.clone(), cpu.clone()).defer();
    let delivery_task = tokio::spawn(delivery_runner.run());

    // When: main retains one driver clone while both handlers retain their own clones.
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let completion_capability = driver.completion_irq_source();
            let event_capability = driver.event_irq_source();
            let completion_source = completion_capability.source();
            let event_source = event_capability.source();
            let completion_handler = stateful_irq_handler(DriverHandler {
                identity: HandlerIdentity::Completion,
                calls: 0,
                driver: driver.clone(),
                port: main_port.clone(),
                source: completion_source,
                report: report_tx.clone(),
            });
            let event_handler = stateful_irq_handler(DriverHandler {
                identity: HandlerIdentity::Event,
                calls: 0,
                driver: driver.clone(),
                port: main_port.clone(),
                source: event_source,
                report: report_tx,
            });
            delivery.register_handlers(|registry| {
                registry.register(completion_vector, completion_handler)?;
                registry.register(event_vector, event_handler)
            })?;
            for (source, vector) in [
                (completion_capability, completion_vector),
                (event_capability, event_vector),
            ] {
                main_port
                    .configure_irq_route(source, IrqRoute::Message(vector))
                    .await?;
                main_port.enable_irq_source(source).await?;
                main_port.unmask_irq_source(source).await?;
            }

            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            driver
                .write_buffer_on(&mut main_port, src, &[2, 4, 6, 8])
                .await?;
            let first = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let first_report = report_rx.recv().await.expect("completion handler reports");
            let fault = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                .await?;
            let fault_report = report_rx.recv().await.expect("event handler reports");
            let final_command = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let final_report = report_rx
                .recv()
                .await
                .expect("completion handler reports again");
            let copied = driver.read_buffer_on(&main_port, dst, 4).await?;
            Ok::<_, ModelError>(MainOutcome {
                commands: [first, fault, final_command],
                sources: [completion_source, event_source],
                observations: vec![first_report, fault_report, final_report],
                copied,
            })
        })
        .await
    });

    let outcome = main.await.expect("software main joins cleanly")?;
    device.request_shutdown().await;
    let delivery_summary = delivery_task.await.expect("host delivery joins cleanly")?;
    let device_summary = worker.await.expect("device worker joins cleanly");

    // Then: either handler can reclaim the shared depth-one credit used by main.
    assert_observations(&outcome, completion_vector, event_vector);
    assert_eq!(outcome.copied, vec![2, 4, 6, 8]);
    assert_eq!(delivery_summary.deliveries(), 3);
    assert_eq!(device_summary.steps, 2);
    assert_eq!(device_summary.errors, 1);
    Ok(())
}

const fn notification_source(notification: IrqNotification) -> IrqSource {
    match notification {
        IrqNotification::Line(snapshot) => snapshot.cause().source(),
        IrqNotification::Message(message) => message.source(),
    }
}

fn assert_observations(
    outcome: &MainOutcome,
    completion_vector: IrqVector,
    event_vector: IrqVector,
) {
    let [first, fault, final_report] = outcome.observations.as_slice() else {
        assert_eq!(outcome.observations.len(), 3);
        return;
    };
    assert_eq!(
        (first.identity, first.call, first.notification.vector()),
        (HandlerIdentity::Completion, 1, completion_vector)
    );
    assert_eq!(
        (fault.identity, fault.call, fault.notification.vector()),
        (HandlerIdentity::Event, 1, event_vector)
    );
    assert_eq!(
        (
            final_report.identity,
            final_report.call,
            final_report.notification.vector()
        ),
        (HandlerIdentity::Completion, 2, completion_vector)
    );
    assert_eq!(notification_source(first.notification), outcome.sources[0]);
    assert_eq!(notification_source(fault.notification), outcome.sources[1]);
    assert_eq!(
        notification_source(final_report.notification),
        outcome.sources[0]
    );
    assert_completion(first, outcome.commands[0]);
    assert_event(fault, outcome.commands[1]);
    assert_completion(final_report, outcome.commands[2]);
}

fn assert_completion(observation: &HandlerObservation, command: CommandId) {
    assert!(matches!(
        observation.records.as_slice(),
        [DriverIrqRecord::Completion(completion)]
            if completion.command_id() == command
                && completion.status() == CompletionStatus::Success
    ));
}

fn assert_event(observation: &HandlerObservation, command: CommandId) {
    assert!(matches!(
        observation.records.as_slice(),
        [DriverIrqRecord::Event(event)]
            if event.command_id() == command
                && event.error_kind() == EventErrorKind::OutOfBounds { len: 4, end: 8 }
    ));
}
