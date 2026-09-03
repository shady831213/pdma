use pdma_model_contract::semantic::{
    BufferId, CommandId, EpochIrqSource, EventErrorKind, EventRecord, IrqSource, ResourceConfig,
    TraceEvent, TraceEventKind,
};

use super::super::support::{
    Case, PipelineRunOutcome, Prepared, drive_to_complete, drive_to_pending,
    prepare_with_resources, trace_kinds,
};
use super::report::{expected_irq, irq_transitions};
use crate::{AsyncStallReason, ModelError};

pub(super) fn pressure_revisits_only_fault_retirement() {
    // Given: one faulting command and zero event-publication slots.
    let mut prepared = prepare_with_resources(
        Case::UnknownBuffer,
        ResourceConfig::default().with_event_publication_slots(0),
    );
    let source = EpochIrqSource::new(
        prepared.queue.epoch(),
        IrqSource::Event(prepared.queue.queue_id()),
    );
    prepared
        .fixture
        .core
        .enable_irq_source(source)
        .expect("enable event IRQ");
    prepared
        .fixture
        .core
        .unmask_irq_source(source)
        .expect("unmask event IRQ");

    // When: initial event pressure and two fault-retry turns remain blocked.
    assert_eq!(
        drive_to_pending(&mut prepared.fixture),
        PipelineRunOutcome::Pending(AsyncStallReason::EventPublicationPressure)
    );
    let retry_wait = AsyncStallReason::FaultRetirementPendingRetry;
    for _ in 0..2 {
        assert_eq!(
            prepared.fixture.step(),
            PipelineRunOutcome::Pending(retry_wait)
        );
    }

    // Then: the command cursor advances once and no fault effect is duplicated.
    assert_pending_state(&mut prepared);
    let pending_cursors = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let pending_irq = prepared.fixture.core.irq_snapshot();
    let pending_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(
        pending_trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionFaulted,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::EventPolled,
        ]
    );
    assert_eq!(pending_irq, None);
    assert_eq!(irq_transitions(&pending_trace), []);
    println!(
        "ADVERSE_MATRIX|route=fault_pressure|phase=pending|cursors={pending_cursors:?}|record=None|record_count=0|irq={pending_irq:?}|trace={pending_trace:?}|fetches=1|executions=1|pending=3|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0"
    );
    release_and_assert(&mut prepared, source);
}

fn assert_pending_state(prepared: &mut Prepared) {
    assert_eq!(prepared.fixture.fetch_count(), 1);
    assert_eq!(
        prepared
            .fixture
            .core
            .queues
            .semantic_cursors(prepared.queue.queue_id()),
        Ok(((1, 1), (0, 0)))
    );
    assert_eq!(
        prepared.fixture.core.poll_queue_event(prepared.queue),
        Ok(None)
    );
    assert_eq!(prepared.fixture.core.irq_snapshot(), None);
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::ExecutionFaulted
        ),
        1
    );
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::EventPublished
        ),
        0
    );
    assert_eq!(
        count(&prepared.fixture.core.trace_events(), TraceEventKind::Stall),
        3
    );
}

fn release_and_assert(prepared: &mut Prepared, source: EpochIrqSource) {
    prepared.fixture.set_resources(ResourceConfig::default());
    let terminal = drive_to_complete(&mut prepared.fixture);
    assert_eq!(terminal, Err(ModelError::UnknownBuffer(BufferId::new(999))));
    let published_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_published_trace(&published_trace);
    let event = prepared
        .fixture
        .core
        .poll_queue_event(prepared.queue)
        .expect("event ring")
        .expect("fault event");
    assert_eq!(
        event,
        EventRecord::new(
            prepared.queue.queue_id(),
            CommandId::new(7),
            EventErrorKind::UnknownBuffer,
            BufferId::new(999),
        )
    );
    assert_eq!(
        prepared.fixture.core.poll_queue_event(prepared.queue),
        Ok(None)
    );
    assert!(prepared.fixture.core.irq_snapshot().is_some());
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::ExecutionFaulted
        ),
        1
    );
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::EventPublished
        ),
        1
    );
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::IrqSourceLatched
        ),
        1
    );
    let final_cursors = prepared
        .fixture
        .core
        .queues
        .semantic_cursors(prepared.queue.queue_id());
    let final_irq = prepared.fixture.core.irq_snapshot();
    let final_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(final_cursors, Ok(((1, 1), (0, 0))));
    assert_eq!(final_irq, Some(expected_irq(source)));
    assert_final_trace(&final_trace);
    let post_terminal = prepared.fixture.step();
    assert_eq!(post_terminal, PipelineRunOutcome::Exhausted);
    println!(
        "ADVERSE_MATRIX|route=fault_pressure|phase=complete|cursors={final_cursors:?}|record={event:?}|record_count=1|irq={final_irq:?}|irq_transitions={:?}|published_trace={published_trace:?}|final_trace={final_trace:?}|fetches=1|executions=1|pending=3|terminal={terminal:?}|post_terminal={post_terminal:?}|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0",
        irq_transitions(&published_trace),
    );
}

fn assert_published_trace(trace: &[TraceEventKind]) {
    assert_eq!(
        trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionFaulted,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::EventPolled,
            TraceEventKind::EventPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
        ]
    );
}

fn assert_final_trace(trace: &[TraceEventKind]) {
    assert_eq!(
        trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionFaulted,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::EventPolled,
            TraceEventKind::EventPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::EventPolled,
            TraceEventKind::EventPolled,
        ]
    );
    assert_eq!(
        irq_transitions(trace),
        [
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
        ]
    );
}

fn count(trace: &[TraceEvent], kind: TraceEventKind) -> usize {
    trace.iter().filter(|event| event.kind() == kind).count()
}
