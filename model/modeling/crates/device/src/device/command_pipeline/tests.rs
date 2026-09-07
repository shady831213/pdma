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
