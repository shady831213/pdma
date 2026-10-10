use std::ops::ControlFlow;

use crate::{Stage, StepOutcome};

/// Binds a continuing value to a child and passes breaking values through unchanged.
#[derive(Debug)]
pub struct OnContinue<S> {
    /// Child hardware used only for continuing inputs.
    pub stage: S,
}

impl<S> OnContinue<S> {
    /// Wraps a child without starting an operation.
    pub const fn new(stage: S) -> Self {
        Self { stage }
    }
}

impl<C, S, B, O> Stage<C> for OnContinue<S>
where
    S: Stage<C, Output = ControlFlow<B, O>>,
{
    type Input = ControlFlow<B, S::Input>;
    type Output = ControlFlow<B, O>;
    type Error = S::Error;
    type Wait = S::Wait;
    type State = ControlFlow<B, S::State>;

    fn start(input: Self::Input) -> Self::State {
        input.map_continue(S::start)
    }

    fn step(
        &mut self,
        context: &mut C,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        match state {
            ControlFlow::Break(output) => StepOutcome::Complete(Ok(ControlFlow::Break(output))),
            ControlFlow::Continue(state) => match self.stage.step(context, state) {
                StepOutcome::Progressed(state) => {
                    StepOutcome::Progressed(ControlFlow::Continue(state))
                }
                StepOutcome::Pending(state, wait) => {
                    StepOutcome::Pending(ControlFlow::Continue(state), wait)
                }
                StepOutcome::Complete(result) => StepOutcome::Complete(result),
            },
        }
    }
}
