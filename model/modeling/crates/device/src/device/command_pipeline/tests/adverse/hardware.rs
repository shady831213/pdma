use hardware_interface_pipeline::{Operation, Stage, StepOutcome};

use crate::ModelError;
use crate::device::{DeviceCore, ModelStepStatus, hardware_context::HardwareContext};
use crate::retirement::stage::RetirementInput;
use crate::retirement::{RetirementPorts, RetirementUnit};

#[test]
fn real_hardware_owners_implement_the_shared_stage_contract() {
    fn assert_stage<S: for<'a> Stage<HardwareContext<'a>, Error = ModelError>>() {}
    fn assert_retirement<S: for<'a> Stage<RetirementPorts<'a>>>() {}
    fn assert_device<S: Stage<(), Error = ModelError>>() {}

    assert_stage::<crate::scheduler::Scheduler>();
    assert_stage::<crate::queue::QueueBlock>();
    assert_stage::<crate::execution::ExecutionUnit>();
    assert_retirement::<RetirementUnit>();
    assert_device::<DeviceCore>();
}

#[test]
fn retirement_hardware_accepts_fresh_retry_operations() {
    // Given: an idle retirement owner with no retained record.
    let mut fixture = super::super::support::Fixture::new(
        pdma_model_contract::semantic::ResourceConfig::default(),
    );

    // When: distinct retry operations reuse the same hardware.
    let first = retire(&mut fixture.core, RetirementInput::Retry);
    let second = retire(&mut fixture.core, RetirementInput::Retry);

    // Then: both requests finish and their completed operations are inert.
    assert_eq!(first, Ok(ModelStepStatus::Progressed));
    assert_eq!(second, Ok(ModelStepStatus::Progressed));
}

pub(super) fn retire(
    core: &mut DeviceCore,
    input: RetirementInput,
) -> crate::ModelResult<ModelStepStatus> {
    let mut operation = Operation::new(input);
    let mut ports = RetirementPorts::new(
        &mut core.queues,
        &core.dma,
        &mut core.irq,
        &mut core.trace,
        core.execution.resources(),
    );
    let StepOutcome::Complete(Ok(feedback)) = operation.step(&mut core.retirement, &mut ports)
    else {
        panic!("retirement should complete");
    };
    assert!(matches!(
        operation.step(&mut core.retirement, &mut ports),
        StepOutcome::Exhausted
    ));
    if let Some(runnable) = feedback.runnable {
        core.scheduler.accept_feedback(runnable);
    }
    feedback.result
}
