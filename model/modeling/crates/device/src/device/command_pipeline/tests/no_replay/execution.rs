use pdma_model_contract::semantic::{
    CommandId, CompletionRecord, CompletionStatus, ResourceConfig, TraceEventKind,
};

use super::super::support::{
    Case, PipelineRunOutcome, drive_to_complete, drive_to_pending, prepare_with_resources, read,
    trace_kinds,
};
use super::report::irq_transitions;
use crate::AsyncStallReason;
use crate::device::ModelStepStatus;

pub(super) fn pressure_revisits_only_execution() {
    // Given: one fetched copy blocked before atomic execution.
    let mut prepared = prepare_with_resources(Case::Copy, ResourceConfig::default());
    assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Progressed);
    prepared
        .fixture
        .set_resources(ResourceConfig::default().with_read_slots(0));

    // When: execution pressure repeats and is then released.
    let wait = AsyncStallReason::ExecutionResourcePressure;
    assert_eq!(
        drive_to_pending(&mut prepared.fixture),
        PipelineRunOutcome::Pending(wait)
    );
    for _ in 0..2 {
        assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Pending(wait));
    }

    // Then: fetch/cursor/memory/record/IRQ state did not replay before one execution.
    assert_eq!(prepared.fixture.fetch_count(), 1);
    let pending_cursors = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let pending_bytes = read(&prepared.fixture.core, prepared.destinations[0]);
    let pending_irq = prepared.fixture.core.irq_snapshot();
    let pending_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(pending_cursors, Ok(((0, 1), (0, 0))));
    assert_eq!(pending_bytes, Ok(vec![0; 8]));
    assert_eq!(pending_irq, None);
    assert_eq!(
        pending_trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
        ]
    );
    prepared.fixture.set_resources(ResourceConfig::default());
    let terminal = drive_to_complete(&mut prepared.fixture);
    assert_eq!(terminal, Ok(ModelStepStatus::Progressed));
    assert_eq!(prepared.fixture.fetch_count(), 1);
    let final_bytes = read(&prepared.fixture.core, prepared.destinations[0]);
    assert_eq!(final_bytes, Ok(vec![1, 2, 3, 4, 0, 0, 0, 0]));
    let completion = prepared
        .fixture
        .core
        .poll_completion(prepared.queue)
        .expect("completion ring");
    assert_eq!(
        completion,
        Some(CompletionRecord::new(
            prepared.queue.queue_id(),
            CommandId::new(7),
            CompletionStatus::Success,
        ))
    );
    assert_eq!(
        prepared.fixture.core.poll_completion(prepared.queue),
        Ok(None)
    );
    let final_cursors = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let final_irq = prepared.fixture.core.irq_snapshot();
    let final_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(final_cursors, Ok(((1, 1), (1, 1))));
    assert!(final_irq.is_some());
    assert_eq!(
        final_trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::ExecutionSucceeded,
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::CompletionPolled,
        ]
    );
    let post_terminal = prepared.fixture.step();
    assert_eq!(post_terminal, PipelineRunOutcome::Exhausted);
    println!(
        "ADVERSE_MATRIX|route=execution_pressure|pending_cursors={pending_cursors:?}|pending_bytes={pending_bytes:?}|pending_record=None|pending_irq={pending_irq:?}|pending_trace={pending_trace:?}|final_cursors={final_cursors:?}|final_bytes={final_bytes:?}|final_record={completion:?}|record_count=1|final_irq={final_irq:?}|irq_transitions={:?}|final_trace={final_trace:?}|fetches=1|executions=1|pending=3|terminal={terminal:?}|post_terminal={post_terminal:?}|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0",
        irq_transitions(&final_trace),
    );
}
