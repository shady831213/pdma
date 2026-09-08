use pdma_model_contract::semantic::{EpochIrqSource, IrqSource, TraceEvent, TraceEventKind};

use super::super::support::{
    PipelineRunOutcome, Prepared, drive_to_complete, drive_to_pending, prepared_pair, trace_kinds,
};
use super::report::{expected_irq, irq_transitions};
use crate::AsyncStallReason;
use crate::device::ModelStepStatus;

pub(super) fn pressure_revisits_only_retirement() {
    // Given: two copies and a one-record completion publication budget.
    let mut prepared = prepared_pair();
    let source = EpochIrqSource::new(
        pdma_model_contract::semantic::DeviceEpoch::initial(),
        IrqSource::Completion(prepared.queue.completion_ring_id()),
    );
    assert_eq!(
        drive_to_complete(&mut prepared.fixture),
        Ok(ModelStepStatus::Progressed)
    );
    assert_eq!(
        crate::device::command_pipeline::tests::support::cursors(
            &prepared.fixture.core,
            prepared.queue
        ),
        ((1, 2), (0, 1))
    );
    prepared.fixture.restart();

    // When: completed DMA waits through two retirement turns.
    let wait = AsyncStallReason::CompletionPublicationPressure;
    assert_eq!(
        drive_to_pending(&mut prepared.fixture),
        PipelineRunOutcome::Pending(wait)
    );
    let pending_trace = prepared.fixture.core.trace_events();
    let pending_irq = prepared.fixture.core.irq_snapshot();
    let pending_cursors = crate::device::command_pipeline::tests::support::cursors(
        &prepared.fixture.core,
        prepared.queue,
    );
    assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Pending(wait));

    // Then: repeated Pending changes only the downstream stall trace.
    assert_pending_unchanged(&prepared, &pending_trace, pending_irq, pending_cursors);
    let repeated_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(pending_irq, Some(expected_irq(source)));
    assert_eq!(
        repeated_trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionSucceeded,
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionSucceeded,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
        ]
    );
    assert_eq!(
        irq_transitions(&repeated_trace),
        [
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
        ]
    );
    println!(
        "ADVERSE_MATRIX|route=completion_pressure|phase=pending|cursors={pending_cursors:?}|record_count=1|irq={pending_irq:?}|trace={repeated_trace:?}|total_fetches=2|pressured_fetches=1|total_executions=2|pressured_executions=1|pending=2|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0",
    );
    release_and_assert(&mut prepared, source);
}

fn assert_pending_unchanged(
    prepared: &Prepared,
    pending_trace: &[TraceEvent],
    pending_irq: Option<pdma_model_contract::semantic::IrqSnapshot>,
    pending_cursors: ((usize, usize), (usize, usize)),
) {
    assert_eq!(prepared.fixture.fetch_count(), 2);
    assert_eq!(pending_cursors, ((2, 2), (0, 1)));
    assert_eq!(
        crate::device::command_pipeline::tests::support::cursors(
            &prepared.fixture.core,
            prepared.queue
        ),
        pending_cursors
    );
    assert_eq!(prepared.fixture.core.irq_snapshot(), pending_irq);
    let repeated_trace = prepared.fixture.core.trace_events();
    assert_eq!(repeated_trace.len(), pending_trace.len() + 1);
    assert_eq!(
        repeated_trace.last().map(TraceEvent::kind),
        Some(TraceEventKind::Stall)
    );
    assert_eq!(
        count(&repeated_trace, TraceEventKind::ExecutionSucceeded),
        2
    );
    assert_eq!(
        count(&repeated_trace, TraceEventKind::CompletionPublished),
        1
    );
    assert_eq!(count(&repeated_trace, TraceEventKind::IrqSourceLatched), 1);
}

fn release_and_assert(prepared: &mut Prepared, source: EpochIrqSource) {
    let first = crate::test_support::poll_completion(&mut prepared.fixture.core, prepared.queue)
        .expect("first record");
    assert_eq!(
        first,
        pdma_model_contract::abi::AbiCompletion::success(u64::from(prepared.queue.index), 7)
    );
    crate::test_support::irq_write(
        &mut prepared.fixture.core,
        prepared.queue,
        0,
        pdma_model_contract::abi::registers::IrqSourceRegisters::ACK.offset(),
        1,
    );
    let terminal = drive_to_complete(&mut prepared.fixture);
    assert_eq!(terminal, Ok(ModelStepStatus::Progressed));
    let second = crate::test_support::poll_completion(&mut prepared.fixture.core, prepared.queue)
        .expect("second record");
    assert_eq!(
        second,
        pdma_model_contract::abi::AbiCompletion::success(u64::from(prepared.queue.index), 99)
    );
    assert_eq!(
        crate::test_support::poll_completion(&mut prepared.fixture.core, prepared.queue),
        None
    );
    assert_eq!(prepared.fixture.fetch_count(), 2);
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::ExecutionSucceeded
        ),
        2
    );
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::CompletionPublished
        ),
        2
    );
    assert_eq!(
        count(
            &prepared.fixture.core.trace_events(),
            TraceEventKind::IrqSourceLatched
        ),
        2
    );
    let final_cursors = crate::device::command_pipeline::tests::support::cursors(
        &prepared.fixture.core,
        prepared.queue,
    );
    let final_irq = prepared.fixture.core.irq_snapshot();
    let final_trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(final_cursors, ((2, 2), (2, 2)));
    assert_eq!(final_irq, Some(expected_irq(source)));
    assert_final_trace(&final_trace);
    let post_terminal = prepared.fixture.step();
    assert_eq!(post_terminal, PipelineRunOutcome::Exhausted);
    println!(
        "ADVERSE_MATRIX|route=completion_pressure|phase=complete|cursors={final_cursors:?}|records=[{first:?},{second:?}]|record_count=2|irq={final_irq:?}|irq_transitions={:?}|trace={final_trace:?}|total_fetches=2|pressured_fetches=1|total_executions=2|pressured_executions=1|pending=2|terminal={terminal:?}|post_terminal={post_terminal:?}|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0",
        irq_transitions(&final_trace),
    );
}

fn assert_final_trace(trace: &[TraceEventKind]) {
    assert_eq!(
        trace,
        [
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::CommandSubmitted,
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionSucceeded,
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::Scheduled,
            TraceEventKind::ExecutionSucceeded,
            TraceEventKind::Stall,
            TraceEventKind::Stall,
            TraceEventKind::CompletionPolled,
            TraceEventKind::IrqDeviceAck,
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::CompletionPolled,
        ]
    );
    assert_eq!(
        irq_transitions(trace),
        [
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::IrqDeviceAck,
            TraceEventKind::IrqSourceLatched,
        ]
    );
}

fn count(trace: &[TraceEvent], kind: TraceEventKind) -> usize {
    trace.iter().filter(|event| event.kind() == kind).count()
}
