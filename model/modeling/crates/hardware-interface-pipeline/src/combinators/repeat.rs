use std::ops::ControlFlow;

use crate::{Stage, StepOutcome};

/// Reuses a child stage until it returns a breaking output.
///
/// Each step advances the child once. Continuing output initializes the next
/// iteration and returns progress; pending retains the current child state.
#[derive(Debug)]
pub struct Repeat<S> {
    /// Persistent hardware reused across iterations and operations.
    pub stage: S,
}

impl<S> Repeat<S> {
    /// Wraps hardware without starting an operation.
    pub const fn new(stage: S) -> Self {
        Self { stage }
    }
}

impl<C, S, I, O> Stage<C> for Repeat<S>
where
    S: Stage<C, Input = I, Output = ControlFlow<O, I>>,
{
    type Input = I;
    type Output = O;
    type Error = S::Error;
    type Wait = S::Wait;
    type State = Option<S::State>;

    fn start(input: Self::Input) -> Self::State {
        Some(S::start(input))
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(child) = state.as_mut() else {
            return StepOutcome::Exhausted;
        };
        match self.stage.step(context, child) {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            StepOutcome::Complete(Ok(ControlFlow::Continue(input))) => {
                *state = Some(S::start(input));
                StepOutcome::Progressed
            }
            StepOutcome::Complete(Ok(ControlFlow::Break(output))) => {
                *state = None;
                StepOutcome::Complete(Ok(output))
            }
            StepOutcome::Complete(Err(error)) => {
                *state = None;
                StepOutcome::Complete(Err(error))
            }
            StepOutcome::Exhausted => {
                *state = None;
                StepOutcome::Exhausted
            }
        }
    }
}
