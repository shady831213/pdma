#![allow(missing_docs)]

use pdma_model_contract::{
    CommandId, CompletionRecord, CompletionStatus, CopyRequest, IrqNotification, IrqRoute,
    IrqSource, IrqVector, ModelError, ModelResult, TraceEvent, TraceEventKind,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};
use pdma_model_host::{AsyncDriver, DriverIrqRecord, IrqDelivery, SoftwareCpu, irq_handler};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoftwarePhase {
    MainBeforeSubmit,
    Handler,
    MainAfterHandler,
}

#[derive(Debug)]
struct HandlerReport {
    notification: IrqNotification,
    records: Vec<DriverIrqRecord>,
}

struct MainOutcome {
    command: CommandId,
    source: IrqSource,
    report: HandlerReport,
    copied: Vec<u8>,
    phase: SoftwarePhase,
}

struct ScenarioResult {
    vector: IrqVector,
    outcome: MainOutcome,
    deliveries: usize,
    device_steps: usize,
    device_trace: Vec<TraceEvent>,
}

#[tokio::test(flavor = "current_thread")]
async fn message_irq_linear_copy_runs_main_handler_main_end_to_end() -> ModelResult<()> {
    // Given: independent device, software-main, host-delivery, and handler actors.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    start_trace(&port).await;
    let cpu = SoftwareCpu::new();
    let phase = cpu.local(SoftwarePhase::MainBeforeSubmit);
    let handler_driver = cpu.local(None::<AsyncDriver>);
    let vector = IrqVector::new(64);
    // Test-only rendezvous: main consumes the handler's observation to prove main-handler-main.
    let (report_tx, report_rx) = oneshot::channel();
    let (delivery, delivery_runner) = IrqDelivery::new(port.clone(), cpu.clone()).defer();
    let delivery_task = tokio::spawn(delivery_runner.run());

    // When: one complete software program configures MSI-style delivery and submits a copy.
    // The program never raises an IRQ itself: completion publication in the device worker
    // latches the source, and host delivery schedules the registered handler.
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let source = IrqSource::Completion(driver.completion_ring_id());
            handler_driver.with(|slot| *slot = Some(driver.clone()))?;
            let handler_driver = handler_driver.clone();
            let handler_phase = phase.clone();
            let handler_port = main_port.clone();
            let mut report = Some(report_tx);
            let handler = irq_handler(move |notification| {
                // Per-turn owned captures: phase/report are test probes; driver_slot/port
                // are specific to this handler's async IRQ service, not universal boilerplate.
                let driver_slot = handler_driver.clone();
                let phase = handler_phase.clone();
                let mut port = handler_port.clone();
                let report = report.take();
                async move {
                    // CpuLocal cannot lend across await, so this handler moves out the driver
                    // for async service and restores it afterward; other handlers may not need it.
                    let Some(mut driver) = driver_slot.with(Option::take)? else {
                        return Err(ModelError::SoftwareCpuStateBusy);
                    };
                    phase.with(|phase| *phase = SoftwarePhase::Handler)?;
                    let records = driver.service_irq_on(&mut port, notification).await;
                    driver_slot.with(|slot| *slot = Some(driver))?;
                    let records = records?;
                    // This report is scenario evidence, not part of IRQ handler mechanics.
                    if let Some(report) = report {
                        drop(report.send(HandlerReport {
                            notification,
                            records,
                        }));
                    }
                    Ok(())
                }
            });
            delivery.register_handler(vector, handler)?;
            main_port
                .configure_irq_route(source, IrqRoute::Message(vector))
                .await?;
            main_port.enable_irq_source(source).await?;
            main_port.unmask_irq_source(source).await?;

            let src = driver.allocate_buffer_on(&mut main_port, 4).await;
            let dst = driver.allocate_buffer_on(&mut main_port, 4).await;
            driver
                .write_buffer_on(&mut main_port, src, &[1, 2, 3, 4])
                .await?;
            let command = driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            let report = report_rx.await.expect("handler returns the completion");
            let phase = phase.with(|phase| {
                assert_eq!(*phase, SoftwarePhase::Handler);
                *phase = SoftwarePhase::MainAfterHandler;
                *phase
            })?;
            let copied = driver.read_buffer_on(&main_port, dst, 4).await?;
            Ok::<_, ModelError>(MainOutcome {
                command,
                source,
                report,
                copied,
                phase,
            })
        })
        .await
    });

    let outcome = main.await.expect("software main joins cleanly")?;
    device.request_shutdown().await;
    let delivery_summary = delivery_task.await.expect("host delivery joins cleanly")?;
    let device_summary = worker.await.expect("device worker joins cleanly");

    // Then: device-triggered message delivery runs handler atomically and returns to main.
    assert_outcome(&ScenarioResult {
        vector,
        outcome,
        deliveries: delivery_summary.deliveries(),
        device_steps: device_summary.steps,
        device_trace: port.trace_events().await,
    });
    Ok(())
}

async fn start_trace(port: &AsyncDeviceHandle) {
    port.enable_tracing().await;
    port.clear_trace().await;
}

fn assert_outcome(result: &ScenarioResult) {
    assert!(matches!(
        result.outcome.report.notification,
        IrqNotification::Message(message)
            if message.vector() == result.vector
                && message.source() == result.outcome.source
    ));
    assert_eq!(result.outcome.report.records.len(), 1);
    let completions = result
        .outcome
        .report
        .records
        .iter()
        .filter_map(|record| match record {
            DriverIrqRecord::Completion(completion) => Some(*completion),
            DriverIrqRecord::Event(_) => None,
        })
        .collect::<Vec<CompletionRecord>>();
    assert_eq!(completions.len(), 1);
    assert_eq!(
        completions
            .first()
            .map(|completion| (completion.command_id(), completion.status())),
        Some((result.outcome.command, CompletionStatus::Success))
    );
    assert_eq!(result.outcome.copied, vec![1, 2, 3, 4]);
    assert_eq!(result.outcome.phase, SoftwarePhase::MainAfterHandler);
    assert_eq!(result.deliveries, 1);
    assert_eq!(result.device_steps, 1);
    assert_trace_order(&result.device_trace);
}

fn assert_trace_order(events: &[TraceEvent]) {
    let expected = [
        TraceEventKind::CompletionPublished,
        TraceEventKind::IrqSourceLatched,
        TraceEventKind::IrqMessageEmitted,
        TraceEventKind::CompletionPolled,
        TraceEventKind::IrqDeviceAck,
    ];
    let observed = events
        .iter()
        .map(TraceEvent::kind)
        .filter(|kind| expected.contains(kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed, expected,
        "completion publication must cause the device-to-handler IRQ lifecycle"
    );
}
