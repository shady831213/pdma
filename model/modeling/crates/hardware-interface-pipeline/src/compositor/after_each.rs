use std::convert::Infallible;

use crate::{Stage, StepOutcome};

/// Runs an always-ready update stage after each upstream interaction, including stalls.
///
/// Both stages run within the same driving boundary. The update cannot finish, fail,
/// or suspend; it preserves the upstream outcome and is skipped on exhaustion.
#[derive(Debug)]
pub struct AfterEach<A, B> {
    /// Hardware whose operation and outcome drive the composition.
    pub upstream: A,
    /// Free-running hardware refreshed after every upstream interaction.
    pub update: B,
}

impl<A, B> AfterEach<A, B> {
    /// Connects hardware without starting an operation.
    pub const fn new(upstream: A, update: B) -> Self {
        Self { upstream, update }
    }
}

impl<C, A, B> Stage<C> for AfterEach<A, B>
where
    A: Stage<C>,
    B: Stage<C, Input = (), Output = Infallible, Error = Infallible, Wait = Infallible>,
{
    type Input = A::Input;
    type Output = A::Output;
    type Error = A::Error;
    type Wait = A::Wait;
    type State = (A::State, B::State);

    fn start(input: Self::Input) -> Self::State {
        (A::start(input), B::start(()))
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let outcome = self.upstream.step(context, &mut state.0);
        if !matches!(outcome, StepOutcome::Exhausted) {
            match self.update.step(context, &mut state.1) {
                StepOutcome::Progressed | StepOutcome::Exhausted => {}
                StepOutcome::Pending(never) | StepOutcome::Complete(Ok(never) | Err(never)) => {
                    match never {}
                }
            }
        }
        outcome
    }
}
