use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, finish},
};
use hardware_interface_pipeline::{
    AfterEach, FanIn, RunError, Stage, StepOutcome, StepResult, Then,
};
use std::convert::Infallible;
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
    // When: the first upstream sweep completes without downstream values.
    for _ in 0..3 {
        assert_eq!(
            hardware.step(&mut context, &mut state),
            Ok(StepOutcome::InProgress)
        );
    }
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
