use pdma_model_contract::{
    CommandId, CompletionRecord, CompletionStatus, IrqNotification, IrqSource, IrqVector,
    ModelResult, QueueConfigHandle, TraceEvent, TraceEventKind,
};
use pdma_model_device::AsyncDeviceHandle;
use pdma_model_host::{
    AsyncDriver, BusDevicePort, BusQueueStatus, CpuLocal, DriverIrqRecord, StatefulIrqHandler,
};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwarePhase {
    MainBeforeSubmit,
    Handler,
    MainAfterHandler,
}

#[derive(Debug)]
pub struct HandlerReport {
    notification: IrqNotification,
    records: Vec<DriverIrqRecord>,
}

#[derive(Debug)]
pub struct LinearCopyHandler {
    pub driver: AsyncDriver,
    pub port: BusDevicePort<AsyncDeviceHandle>,
    pub phase: CpuLocal<SoftwarePhase>,
    pub report: Option<oneshot::Sender<HandlerReport>>,
}

impl StatefulIrqHandler for LinearCopyHandler {
    async fn handle(&mut self, notification: IrqNotification) -> ModelResult<()> {
        self.phase.with(|phase| *phase = SoftwarePhase::Handler)?;
        let records = self
            .driver
            .service_irq_on(&mut self.port, notification)
            .await?;
        // This report is scenario evidence, not part of IRQ handler mechanics.
        if let Some(report) = self.report.take() {
            drop(report.send(HandlerReport {
                notification,
                records,
            }));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct MainOutcome {
    pub command: CommandId,
    pub queue: QueueConfigHandle,
    pub source: IrqSource,
    pub report: HandlerReport,
    pub copied: Vec<u8>,
    pub phase: SoftwarePhase,
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub vector: IrqVector,
    pub outcome: MainOutcome,
    pub queue_status: BusQueueStatus,
    pub deliveries: usize,
    pub device_steps: usize,
    pub device_trace: Vec<TraceEvent>,
}

pub async fn start_trace(port: &AsyncDeviceHandle) {
    port.enable_tracing().await;
    port.clear_trace().await;
}

/// # Panics
///
/// Panics when the completed scenario violates its end-to-end ABI or IRQ invariants.
pub fn assert_outcome(result: &ScenarioResult) {
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
    assert_queue_cursors(result.queue_status);
    let lifecycle = assert_lifecycle_order(result);
    emit_manual_qa(result, &lifecycle);
}

fn assert_queue_cursors(status: BusQueueStatus) {
    assert_eq!(status.submission().producer().raw(), 1);
    assert_eq!(status.captured_submission_producer().raw(), 1);
    assert_eq!(status.submission().consumer().raw(), 1);
    assert_eq!(status.completion().producer().raw(), 1);
    assert_eq!(status.completion().consumer().raw(), 1);
    assert_eq!(status.event().producer().raw(), 0);
    assert_eq!(status.event().consumer().raw(), 0);
}

#[derive(Clone, Copy)]
enum LifecycleObservation {
    SubmissionProducer(u64),
    CapturedProducer {
        producer: u64,
        captured: u64,
    },
    DeviceTrace(TraceEventKind),
    CompletionConsumer {
        producer: u64,
        consumer: u64,
        record_count: usize,
    },
}

impl LifecycleObservation {
    fn asserted_kind(self) -> TraceEventKind {
        match self {
            Self::SubmissionProducer(producer) => {
                assert_eq!(producer, 1, "submission producer must publish one command");
                TraceEventKind::CommandSubmitted
            }
            Self::CapturedProducer { producer, captured } => {
                assert_eq!(captured, producer, "doorbell must capture the producer");
                TraceEventKind::DoorbellRunnable
            }
            Self::DeviceTrace(kind) => kind,
            Self::CompletionConsumer {
                producer,
                consumer,
                record_count,
            } => {
                assert_eq!(consumer, producer, "handler must drain the completion");
                assert_eq!(record_count, 1, "handler must report the drained record");
                TraceEventKind::CompletionPolled
            }
        }
    }
}

fn assert_lifecycle_order(result: &ScenarioResult) -> Vec<TraceEventKind> {
    let expected = [
        TraceEventKind::CommandSubmitted,
        TraceEventKind::DoorbellRunnable,
        TraceEventKind::Scheduled,
        TraceEventKind::ExecutionSucceeded,
        TraceEventKind::CompletionPublished,
        TraceEventKind::IrqSourceLatched,
        TraceEventKind::IrqMessageEmitted,
        TraceEventKind::CompletionPolled,
        TraceEventKind::IrqDeviceAck,
    ];
    let expected_device_trace = [
        TraceEventKind::Scheduled,
        TraceEventKind::ExecutionSucceeded,
        TraceEventKind::CompletionPublished,
        TraceEventKind::IrqSourceLatched,
        TraceEventKind::IrqMessageEmitted,
        TraceEventKind::IrqDeviceAck,
    ];
    let observed_device_trace = result
        .device_trace
        .iter()
        .map(TraceEvent::kind)
        .filter(|kind| expected_device_trace.contains(kind))
        .collect::<Vec<_>>();
    assert_eq!(observed_device_trace, expected_device_trace);
    let status = result.queue_status;
    let submitted = LifecycleObservation::SubmissionProducer(status.submission().producer().raw());
    let doorbell = LifecycleObservation::CapturedProducer {
        producer: status.submission().producer().raw(),
        captured: status.captured_submission_producer().raw(),
    };
    let completion_poll = LifecycleObservation::CompletionConsumer {
        producer: status.completion().producer().raw(),
        consumer: status.completion().consumer().raw(),
        record_count: result.outcome.report.records.len(),
    };
    let observed = std::iter::once(submitted)
        .chain(std::iter::once(doorbell))
        .chain(
            observed_device_trace
                .iter()
                .copied()
                .take(5)
                .map(LifecycleObservation::DeviceTrace),
        )
        .chain(std::iter::once(completion_poll))
        .chain(
            observed_device_trace
                .iter()
                .copied()
                .skip(5)
                .map(LifecycleObservation::DeviceTrace),
        )
        .map(LifecycleObservation::asserted_kind)
        .collect::<Vec<_>>();
    assert_eq!(
        observed, expected,
        "trace, queue cursors, and handler records must prove the complete lifecycle"
    );
    observed
}

fn emit_manual_qa(result: &ScenarioResult, lifecycle: &[TraceEventKind]) {
    let status = result.queue_status;
    println!(
        "manual-qa asserted lifecycle: {}",
        lifecycle
            .iter()
            .map(|kind| format!("{kind:?}"))
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    println!(
        "manual-qa ABI cursors: producer={} captured={} submission_consumer={} completion_producer={} completion_consumer={}",
        status.submission().producer().raw(),
        status.captured_submission_producer().raw(),
        status.submission().consumer().raw(),
        status.completion().producer().raw(),
        status.completion().consumer().raw()
    );
    println!(
        "manual-qa Message vector={} deliveries={} device_steps={} phase={:?} destination={:?}",
        result.vector.raw(),
        result.deliveries,
        result.device_steps,
        result.outcome.phase,
        result.outcome.copied,
    );
}
