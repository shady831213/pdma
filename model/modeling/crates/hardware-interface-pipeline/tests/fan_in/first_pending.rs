use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context},
};
use hardware_interface_pipeline::{FanIn, Operation, RunError, StepOutcome};
#[test]
fn first_incomplete_output_retains_work_and_explicit_reset_drops_it() {
    // Given: a candidate is selected and its output accepts a job on the first step.
    let mut hardware = FanIn::new(vec![Candidate(0)], Policy::Fixed, Accept::Normal);
    let mut operation = Operation::new(vec![()]);
    let mut context = Context::with_jobs(&[0]);
    context.blocked.push(0);
    // When: the output stalls across sweeps then the owner cancels the invocation.
    for _ in 0..9 {
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    operation.reset();
    // Then: accepted external effects remain and no invocation work can restart.
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    );
    assert_eq!(context.accepted, [0]);
    assert_eq!(context.observed, [0]);
}
