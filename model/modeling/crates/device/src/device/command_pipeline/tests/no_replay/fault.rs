use pdma_model_contract::semantic::{
    BufferId, CommandId, EpochIrqSource, IrqSource, ResourceConfig, TraceEvent, TraceEventKind,
};

use super::super::support::{
    Case, PipelineRunOutcome, Prepared, drive_to_complete, drive_to_pending,
    prepare_with_resources, trace_kinds,
};
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
    release_and_assert(&mut prepared);
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

fn release_and_assert(prepared: &mut Prepared) {
    prepared.fixture.set_resources(ResourceConfig::default());
    assert_eq!(
        drive_to_complete(&mut prepared.fixture),
        Err(ModelError::UnknownBuffer(BufferId::new(999)))
    );
    assert_eq!(
        trace_kinds(&prepared.fixture.core.trace_events()),
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
    let event = prepared
        .fixture
        .core
        .poll_queue_event(prepared.queue)
        .expect("event ring")
        .expect("fault event");
    assert_eq!(event.command_id(), CommandId::new(7));
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
    assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Exhausted);
}

fn count(trace: &[TraceEvent], kind: TraceEventKind) -> usize {
    trace.iter().filter(|event| event.kind() == kind).count()
}
