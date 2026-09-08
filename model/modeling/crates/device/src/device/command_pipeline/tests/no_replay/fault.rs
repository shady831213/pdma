use super::super::support::{
    Case, PipelineRunOutcome, drive_to_complete, drive_to_pending, prepare, trace_kinds,
};
use crate::{AsyncStallReason, ModelError, test_support};
use pdma_model_contract::abi::{AbiEventKind, registers::IrqSourceRegisters};
use pdma_model_contract::semantic::TraceEventKind;

pub(super) fn pressure_revisits_only_fault_retirement() {
    // Given: a full exact event ring and another captured faulting descriptor.
    let mut prepared = prepare(Case::UnknownBuffer);
    test_support::irq_write(
        &mut prepared.fixture.core,
        prepared.queue,
        1,
        IrqSourceRegisters::ENABLED.offset(),
        1,
    );
    test_support::irq_write(
        &mut prepared.fixture.core,
        prepared.queue,
        1,
        IrqSourceRegisters::MASKED.offset(),
        0,
    );
    let source = ModelError::DmaSubmissionState;
    assert_eq!(drive_to_complete(&mut prepared.fixture), Err(source));
    test_support::submit(
        &mut prepared.fixture.core,
        prepared.queue,
        test_support::copy(8, 0xdead_0000, prepared.destinations[0].base().raw(), 4),
    );
    test_support::doorbell(&mut prepared.fixture.core, prepared.queue);
    prepared.fixture.restart();
    assert_eq!(
        drive_to_pending(&mut prepared.fixture),
        PipelineRunOutcome::Pending(AsyncStallReason::EventPublicationPressure)
    );
    let before = trace_kinds(&prepared.fixture.core.trace_events());

    // When: retained retirement retries against the full event ring.
    for _ in 0..3 {
        assert_eq!(
            prepared.fixture.step(),
            PipelineRunOutcome::Pending(AsyncStallReason::FaultRetirementPendingRetry)
        );
    }

    // Then: neither the descriptor nor execution replays, and freeing one slot publishes once.
    assert_eq!(prepared.fixture.fetch_count(), 2);
    assert_eq!(
        super::super::support::cursors(&prepared.fixture.core, prepared.queue),
        ((2, 2), (0, 0))
    );
    let pending = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(pending.get(..before.len()), Some(before.as_slice()));
    assert_eq!(
        pending.get(before.len()..),
        Some([TraceEventKind::Stall; 3].as_slice())
    );
    let first =
        test_support::poll_event(&mut prepared.fixture.core, prepared.queue).expect("first event");
    assert_eq!(first.command_id, 7);
    test_support::irq_write(
        &mut prepared.fixture.core,
        prepared.queue,
        1,
        IrqSourceRegisters::ACK.offset(),
        1,
    );
    assert_eq!(drive_to_complete(&mut prepared.fixture), Err(source));
    let second =
        test_support::poll_event(&mut prepared.fixture.core, prepared.queue).expect("second event");
    assert_eq!(second.command_id, 8);
    assert_eq!(second.fault().kind(), AbiEventKind::UnmappedDmaAddress);
    assert_eq!(second.fault_locator, 0xdead_0000);
    assert_eq!(
        test_support::poll_event(&mut prepared.fixture.core, prepared.queue),
        None
    );
    let trace = trace_kinds(&prepared.fixture.core.trace_events());
    assert_eq!(
        trace
            .iter()
            .filter(|kind| **kind == TraceEventKind::EventPublished)
            .count(),
        2
    );
    assert!(!trace.contains(&TraceEventKind::ExecutionSucceeded));
    assert_eq!(
        trace
            .iter()
            .filter(|kind| **kind == TraceEventKind::ExecutionFaulted)
            .count(),
        2
    );
    assert_eq!(
        super::report::irq_transitions(&trace),
        [
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::IrqDeviceAck,
            TraceEventKind::IrqSourceLatched,
        ]
    );
    assert!(prepared.fixture.core.irq_snapshot().is_some());
    assert_eq!(prepared.fixture.step(), PipelineRunOutcome::Exhausted);
    assert_eq!(prepared.fixture.fetch_count(), 2);
    println!(
        "ADVERSE_MATRIX|route=fault_pressure|fetches=2|executions=2|events=7,8|pending=4|cursors=2/2|duplicate_fetch_delta=0|duplicate_execution_delta=0|duplicate_record_delta=0"
    );
}
