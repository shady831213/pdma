use crate::{BranchError, Stage, StepOutcome};

use super::branches::Branches;

/// Application-owned input planning and reduction for one fan-in operation.
pub trait Combine<C, S: Stage<C, Input = ()>> {
    /// Aggregate returned after all selected inputs complete.
    type Output;
    /// Planning and reduction state retained across steps.
    type State;
    /// Starts one operation without interacting with its sources.
    fn start() -> Self::State;
    /// Selects an ordered subset of sources without consuming input.
    fn plan(
        &mut self,
        context: &C,
        sources: &[S],
        state: &mut Self::State,
    ) -> StepOutcome<Vec<usize>, BranchError<S::Error>, S::Wait>;
    /// Reduces owned outputs in plan order.
    ///
    /// # Errors
    /// Returns the application's aggregate failure.
    fn combine(
        &mut self,
        state: &mut Self::State,
        outputs: Vec<S::Output>,
    ) -> Result<Self::Output, BranchError<S::Error>>;
}

/// Gathers application-selected inputs and reduces their owned outputs.
#[derive(Debug)]
pub struct FanIn<S, P> {
    pub(super) sources: Vec<S>,
    policy: P,
}

/// Retained planning, accepted child work, and partial aggregate.
#[derive(Debug)]
pub struct FanInState<P, T, O> {
    policy: P,
    branches: Option<Branches<(), T, O>>,
    done: bool,
}

impl<S, P> FanIn<S, P> {
    /// Owns the sources and application policy for the hardware lifetime.
    pub const fn new(sources: Vec<S>, policy: P) -> Self {
        Self { sources, policy }
    }
}

impl<C, S, P> Stage<C> for FanIn<S, P>
where
    S: Stage<C, Input = ()>,
    P: Combine<C, S>,
{
    type Input = ();
    type Output = P::Output;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = FanInState<P::State, S::State, S::Output>;

    fn start((): ()) -> Self::State {
        FanInState {
            policy: P::start(),
            branches: None,
            done: false,
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if state.done {
            return StepOutcome::Exhausted;
        }
        if state.branches.is_none() {
            let plan = match self.policy.plan(context, &self.sources, &mut state.policy) {
                StepOutcome::Progressed => return StepOutcome::Progressed,
                StepOutcome::Pending(wait) => return StepOutcome::Pending(wait),
                StepOutcome::Complete(Ok(plan)) => plan,
                StepOutcome::Complete(Err(error)) => {
                    state.done = true;
                    return StepOutcome::Complete(Err(error));
                }
                StepOutcome::Exhausted => {
                    state.done = true;
                    return StepOutcome::Exhausted;
                }
            };
            match Branches::new(
                plan.into_iter().map(|index| (index, ())).collect(),
                self.sources.len(),
            ) {
                Ok(branches) => state.branches = Some(branches),
                Err(error) => {
                    state.done = true;
                    return StepOutcome::Complete(Err(error));
                }
            }
        }
        let Some(branches) = &mut state.branches else {
            return StepOutcome::Exhausted;
        };
        match branches.step(&mut self.sources, context) {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            StepOutcome::Complete(result) => {
                state.done = true;
                StepOutcome::Complete(
                    result.and_then(|outputs| self.policy.combine(&mut state.policy, outputs)),
                )
            }
            StepOutcome::Exhausted => {
                state.done = true;
                StepOutcome::Exhausted
            }
        }
    }
}
