#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "checked unit-test fixtures"
)]

use hardware_interface_pipeline::{Operation, StepOutcome};
use pdma_model_contract::semantic::ResourceConfig;

use crate::device::{DeviceCore, ModelStepStatus};

mod adverse;
mod composition;
mod no_replay;
mod parity;
mod shared_queue_state;
mod support;

#[tokio::test(flavor = "current_thread")]
async fn drive_policy_parity() {
    parity::drive_policy_parity().await;
}

#[test]
fn no_replay() {
    no_replay::no_replay();
}

#[test]
fn completed_operation_is_inert_and_device_accepts_another_operation() {
    // Given: an idle device and one accepted generic operation.
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    );
    core.enable_tracing();
    let mut operation = Operation::new(());
    assert_eq!(
        operation.step(&mut core, &mut ()),
        StepOutcome::Complete(Ok(ModelStepStatus::Idle))
    );
    let trace = core.trace_events();
    assert_eq!(
        support::trace_kinds(&trace),
        [pdma_model_contract::semantic::TraceEventKind::Idle]
    );

    // When: the completed operation is revisited, then the hardware is reused.
    assert_eq!(operation.step(&mut core, &mut ()), StepOutcome::Exhausted);
    assert_eq!(core.trace_events(), trace);
    let mut next = Operation::new(());

    // Then: a fresh operation completes without any retained exhausted state.
    assert_eq!(
        next.step(&mut core, &mut ()),
        StepOutcome::Complete(Ok(ModelStepStatus::Idle))
    );
    assert!(core.command_pipeline.is_empty());
}

#[test]
fn undoorbelled_submission_keeps_three_interactions_without_idle_trace() {
    // Given: submitted work exists but its queue has not been made runnable.
    let mut prepared = support::prepare(support::Case::Copy);
    prepared.fixture.core.runnable.reset();
    let trace = prepared.fixture.core.trace_events();

    // When: one root request traverses the empty command path.
    assert_eq!(
        prepared.fixture.step(),
        support::PipelineRunOutcome::Progressed
    );
    assert_eq!(
        prepared.fixture.step(),
        support::PipelineRunOutcome::Progressed
    );
    assert_eq!(
        prepared.fixture.step(),
        support::PipelineRunOutcome::Complete(Ok(ModelStepStatus::Idle))
    );

    // Then: the unsubmitted doorbell causes no fetch, trace, or queue consumption.
    assert_eq!(prepared.fixture.fetch_count(), 0);
    assert_eq!(prepared.fixture.core.trace_events(), trace);
    assert_eq!(
        prepared
            .fixture
            .core
            .queues
            .semantic_cursors(prepared.queue.queue_id()),
        Ok(((0, 1), (0, 0)))
    );
    assert!(prepared.fixture.core.command_pipeline.is_empty());
}
