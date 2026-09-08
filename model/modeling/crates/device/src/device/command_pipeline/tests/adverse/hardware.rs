use hardware_interface_pipeline::{Stage, StepOutcome};

use crate::ModelError;
use crate::device::retirement::RetirementUnit;
use crate::device::retirement::stage::RetirementState;
use crate::device::{DeviceCore, ModelStepStatus, context::HardwareContext};

#[test]
fn real_hardware_owners_implement_the_shared_stage_contract() {
    fn assert_stage<S: for<'a> Stage<HardwareContext<'a>, Error = ModelError>>() {}
    fn assert_device<S: Stage<(), Error = ModelError>>() {}

    assert_stage::<crate::device::issue::IssueUnit>();
    assert_stage::<crate::device::execution::ExecutionUnit>();
    assert_stage::<RetirementUnit>();
    assert_device::<DeviceCore>();
}

#[test]
fn retirement_hardware_accepts_fresh_retry_operations() {
    // Given: an idle retirement owner with no retained record.
    let mut fixture = super::super::support::Fixture::new(
        pdma_model_contract::semantic::ResourceConfig::default(),
    );

    // When: distinct retry operations reuse the same hardware.
    let first = retire(&mut fixture.core, RetirementState::Retry);
    let second = retire(&mut fixture.core, RetirementState::Retry);

    // Then: both requests finish and their completed operations are inert.
    assert_eq!(first, Ok(ModelStepStatus::Progressed));
    assert_eq!(second, Ok(ModelStepStatus::Progressed));
}

pub(super) fn retire(
    core: &mut DeviceCore,
    mut state: RetirementState,
) -> crate::ModelResult<ModelStepStatus> {
    let mut context = HardwareContext {
        queues: &mut core.queues,
        runnable: &mut core.runnable,
        memory: &core.memory,
        irq: &mut core.irq,
        trace: &mut core.trace,
        fetches: &mut core.native_fetches,
        resources: core.execution.resources(),
    };
    let StepOutcome::Complete(result) = core.retirement.step(&mut context, &mut state) else {
        panic!("retirement should complete");
    };
    assert!(matches!(
        core.retirement.step(&mut context, &mut state),
        StepOutcome::Exhausted
    ));
    result
}
