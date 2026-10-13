use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, Job, finish},
};
use hardware_interface_pipeline::{
    AfterEach, BranchError, FanIn, HardwareDriver, RunError, Stage, StepOutcome, StepResult, Then,
};
use std::convert::Infallible;
struct Inputs;
impl Stage<Context> for Inputs {
    type Input = ();
    type Output = Vec<()>;
    type Error = BranchError<&'static str>;
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, _: &mut Context, (): &mut ()) -> StepResult<Self::Output, Self::Error> {
        Ok(StepOutcome::Complete(Ok(vec![(); 3])))
    }
}
struct Next;
impl Stage<Context> for Next {
    type Input = Vec<(usize, Option<Job>)>;
    type Output = ();
    type Error = BranchError<&'static str>;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        let receipts = state.take().ok_or(RunError::Exhausted)?;
        assert!(receipts.iter().all(|(_, job)| job.is_some()));
        context.updates += 1;
        Ok(StepOutcome::Complete(Ok(())))
    }
}
#[test]
fn external_driver_preserves_round_robin_across_finite_invocations() {
    // Given: each unit-input service composes one finite fan and receipt collection.
    let fan = FanIn::new(
        (0..3).map(Candidate).collect(),
        Policy::RoundRobin(1),
        Accept::Normal,
    );
    let mut hardware = Then::new(Inputs, Then::new(fan, Next));
    let mut driver = HardwareDriver::new();
    let mut context = Context::with_jobs(&[0, 1, 2]);
    // When: the external driver advances until three finite invocations finish.
    for _ in 0..100 {
        assert_eq!(
            driver.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
        if context.updates == 3 {
            break;
        }
    }
    // Then: only one acceptance per invocation and hardware policy position survives.
    assert_eq!(context.updates, 3);
    assert_eq!(context.accepted, [1, 2, 0]);
}

struct Identity;
impl Stage<Context> for Identity {
    type Input = Option<usize>;
    type Output = Option<usize>;
    type Error = &'static str;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut Context,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        Ok(StepOutcome::Complete(Ok(state
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}
struct Update;
impl Stage<Context> for Update {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = bool;
    fn start((): ()) -> bool {
        false
    }
    fn step(&mut self, context: &mut Context, done: &mut bool) -> StepResult<(), Infallible> {
        if *done {
            return Err(RunError::Exhausted);
        }
        *done = true;
        context.updates += 1;
        Ok(StepOutcome::Complete(Ok(())))
    }
}
#[test]
fn composed_sources_finish_before_candidate_values_reach_policy() {
    // Given: each source has two value-producing stages.
    let mut hardware = FanIn::new(
        (0..2).map(|i| Then::new(Candidate(i), Identity)).collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut state =
        <FanIn<Then<Candidate, Identity>, Policy, Accept> as Stage<Context>>::start(vec![(); 2]);
    let mut context = Context::with_jobs(&[1]);
    // When: the first input/plan/output sweep has no completed composite sources yet.
    for _ in 0..3 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
    assert_eq!(context.observed, [0, 1]);
    assert!(context.accepted.is_empty());
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: composed source boundaries are preserved before acceptance.
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    assert_eq!(context.accepted, [1]);
    assert_eq!(context.observed, [0, 1]);
}
#[test]
fn after_each_sources_finish_finite_maintenance_per_source_step() {
    // Given: each candidate has its own finite maintenance stage.
    let mut hardware = FanIn::new(
        (0..2)
            .map(|i| AfterEach::new(Candidate(i), Update))
            .collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut state =
        <FanIn<AfterEach<Candidate, Update>, Policy, Accept> as Stage<Context>>::start(vec![(); 2]);
    let mut context = Context::with_jobs(&[1]);
    // When: all source values are collected and selected work executes.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: both source invocations finish maintenance, only the selected output consumes work.
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    assert_eq!(context.updates, 2);
    assert_eq!(context.accepted, [1]);
}
