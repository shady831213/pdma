use crate::TestResult;
use crate::support::{Fixture, allocate_pair};
use pdma_model_contract::{CommandId, DeviceEpoch, RingCursor, TraceEventKind};

/// State-derived publication-pressure observation fields.
pub type Observation = (usize, usize, usize, bool, bool, bool, bool, bool);

/// Drives one cached retirement through publication retry.
///
/// # Panics
///
/// Panics when the exact fault ring does not contain the published record.
pub async fn observe() -> TestResult<Observation> {
    let mut fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (source, first) = allocate_pair(&fixture.handle).await?;
    let (_, second) = allocate_pair(&fixture.handle).await?;
    fixture.handle.enable_tracing().await;
    let irq = fixture.completion_source();
    fixture.handle.enable_irq_source(irq).await?;
    fixture.handle.unmask_irq_source(irq).await?;
    fixture
        .submit_copy(0, 80, source.base(), first.base())
        .await?;
    fixture
        .submit_copy(1, 81, source.base(), second.base())
        .await?;
    fixture.capture(2).await?;
    let worker = fixture.start()?;
    fixture.handle.wait_for_stall_after(0).await;
    let before = fixture.status().await?;
    let pending = usize::from(fixture.handle.pending_dma_result().await.is_some());
    let before_bytes = fixture.handle.read_memory(fixture.completion, 32).await?;
    let before_trace = fixture.handle.trace_events().await;
    let executions = before_trace
        .iter()
        .filter(|event| {
            event.kind() == TraceEventKind::ExecutionSucceeded
                && event.command_id() == Some(CommandId::new(81))
        })
        .count();
    fixture.consume_completion(1).await?;
    fixture.handle.wait_for_progress_after(1).await;
    let after = fixture.status().await?;
    let after_bytes = fixture.handle.read_memory(fixture.completion, 32).await?;
    let after_trace = fixture.handle.trace_events().await;
    let retries = before_trace
        .iter()
        .filter(|event| {
            event.kind() == TraceEventKind::Stall
                && event.detail() == "CompletionPublicationPressure"
        })
        .count();
    let publication = after_trace
        .iter()
        .position(|event| event.kind() == TraceEventKind::CompletionPublished);
    let latch = after_trace
        .iter()
        .position(|event| event.kind() == TraceEventKind::IrqSourceLatched);
    let fault_count = after_trace
        .iter()
        .filter(|event| event.kind() == TraceEventKind::ExecutionFaulted)
        .count();
    let result = (
        pending,
        executions,
        retries,
        before_bytes != after_bytes,
        publication
            .zip(latch)
            .is_some_and(|(publish, irq)| publish < irq),
        fault_count > 1,
        after.submission().consumer() == RingCursor::new(2),
        before.completion().producer() == RingCursor::new(1),
    );
    fixture.handle.request_shutdown().await;
    worker.await?;

    Ok((
        result.0,
        result.1,
        result.2,
        result.3,
        result.4,
        fault_reexecuted().await?,
        result.6,
        result.7,
    ))
}

async fn fault_reexecuted() -> TestResult<bool> {
    let mut fault_fixture = Fixture::new(DeviceEpoch::initial(), 1).await?;
    let (fault_source, _) = allocate_pair(&fault_fixture.handle).await?;
    fault_fixture.handle.enable_tracing().await;
    fault_fixture
        .submit_copy(
            0,
            82,
            fault_source.base(),
            pdma_model_contract::DmaAddress::new(u64::MAX),
        )
        .await?;
    fault_fixture.capture(1).await?;
    let fault_worker = fault_fixture.start()?;
    fault_fixture.handle.wait_for_progress_after(1).await;
    let fault_trace = fault_fixture.handle.trace_events().await;
    let fault_publications = fault_trace
        .iter()
        .filter(|event| event.kind() == TraceEventKind::EventPublished)
        .count();
    let fault_executions = fault_fixture.handle.native_steps().await;
    let event_bytes = fault_fixture
        .handle
        .read_memory(fault_fixture.event, 64)
        .await?;
    assert!(event_bytes.iter().any(|byte| *byte != 0));
    fault_fixture.handle.request_shutdown().await;
    fault_worker.await?;
    Ok(fault_executions != 1 || fault_publications != 1)
}
