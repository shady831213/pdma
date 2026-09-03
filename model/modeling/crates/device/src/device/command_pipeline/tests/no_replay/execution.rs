use pdma_model_contract::semantic::{ResourceConfig, TraceEventKind};

use super::super::support::{
    Case, PipelineRunOutcome, drive_to_complete, drive_to_pending, prepare_with_resources, read,
    trace_kinds,
};
use crate::AsyncStallReason;
use crate::device::ModelStepStatus;

pub(super) fn pressure_revisits_only_execution() {
    // Given: one fetched copy blocked before atomic execution.
    let mut prepared =
        prepare_with_resources(Case::Copy, ResourceConfig::default().with_read_slots(0));

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
    assert_eq!(
        prepared
            .fixture
            .core
            .queues
            .semantic_cursors(prepared.queue.queue_id()),
        Ok(((0, 1), (0, 0)))
    );
    assert_eq!(
        read(&prepared.fixture.core, prepared.destinations[0]),
        Ok(vec![0; 8])
    );
    assert_eq!(prepared.fixture.core.irq_snapshot(), None);
    assert_eq!(
        trace_kinds(&prepared.fixture.core.trace_events()),
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
    assert_eq!(
        drive_to_complete(&mut prepared.fixture),
        Ok(ModelStepStatus::Progressed)
    );
    assert_eq!(prepared.fixture.fetch_count(), 1);
    assert_eq!(
        read(&prepared.fixture.core, prepared.destinations[0]),
        Ok(vec![1, 2, 3, 4, 0, 0, 0, 0])
    );
    assert!(
        prepared
            .fixture
            .core
            .poll_completion(prepared.queue)
            .expect("completion ring")
            .is_some()
    );
    assert_eq!(
        prepared.fixture.core.poll_completion(prepared.queue),
        Ok(None)
    );
    assert_eq!(
        trace_kinds(&prepared.fixture.core.trace_events()),
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
    assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Exhausted);
}
