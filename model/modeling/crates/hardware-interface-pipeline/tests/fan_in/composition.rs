use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, Job, finish},
};
use hardware_interface_pipeline::{
    BranchError, FanIn, Repeat, RunError, Stage, StepOutcome, StepResult, Then,
};
use std::ops::ControlFlow;
struct Next;
impl Stage<Context> for Next {
    type Input = Vec<(usize, Option<Job>)>;
    type Output = ControlFlow<usize, Vec<()>>;
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
        Ok(StepOutcome::Complete(Ok(if context.accepted.len() == 3 {
            ControlFlow::Break(3)
        } else {
            ControlFlow::Continue(vec![(); 3])
        })))
    }
}
#[test]
fn then_and_repeat_preserve_round_robin_across_iteration_state() {
    // Given: a finite fan is composed into a sustained three-round operation.
    let fan = FanIn::new(
        (0..3).map(Candidate).collect(),
        Policy::RoundRobin(1),
        Accept::Normal,
    );
    let mut hardware = Repeat::new(Then::new(fan, Next));
    let mut state =
        <Repeat<Then<FanIn<Candidate, Policy, Accept>, Next>> as Stage<Context>>::start(vec![
            ();
            3
        ]);
    let mut context = Context::with_jobs(&[0, 1, 2]);
    // When: repeated batches reach the application break.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: only one acceptance per batch and hardware policy position survives.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(3))));
    assert_eq!(context.accepted, [1, 2, 0]);
}
