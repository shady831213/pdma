use std::convert::Infallible;

use crate::{ReadyStage, Stage, StepOutcome};

/// Runs an always-ready update stage after each upstream interaction, including stalls.
///
/// Both stages run within the same driving boundary. The update cannot finish, fail,
/// or suspend; it preserves the upstream outcome.
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
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        let outcome = self.upstream.step(context, state.0);
        let update = match self.update.step(context, state.1) {
            StepOutcome::Progressed(update) => update,
            StepOutcome::Pending(_, never) | StepOutcome::Complete(Ok(never) | Err(never)) => {
                match never {}
            }
        };
        match outcome {
            StepOutcome::Progressed(state) => StepOutcome::Progressed((state, update)),
            StepOutcome::Pending(state, wait) => StepOutcome::Pending((state, update), wait),
            StepOutcome::Complete(result) => StepOutcome::Complete(result),
        }
    }
}

impl<C, A, B> ReadyStage<C> for AfterEach<A, B>
where
    A: ReadyStage<C>,
    B: Stage<C, Input = (), Output = Infallible, Error = Infallible, Wait = Infallible>,
{
    fn ready(&self, context: &C) -> bool {
        self.upstream.ready(context)
    }
}
