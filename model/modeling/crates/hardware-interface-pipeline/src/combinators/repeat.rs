use crate::{RunError, Stage, StepOutcome, StepResult};
use std::ops::ControlFlow;

/// Reuses hardware while each completed child transfers its next input.
#[derive(Debug)]
pub struct Repeat<S> {
    /// Hardware reused between iterations.
    pub stage: S,
}
impl<S> Repeat<S> {
    /// Creates a repetition without starting an invocation.
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
    type State = Option<S::State>;
    fn start(input: I) -> Self::State {
        Some(S::start(input))
    }
    fn step(&mut self, context: &mut C, state: &mut Self::State) -> StepResult<O, S::Error> {
        let mut child = state.take().ok_or(RunError::Exhausted)?;
        Ok(match self.stage.step(context, &mut child)? {
            StepOutcome::InProgress => {
                *state = Some(child);
                StepOutcome::InProgress
            }
            StepOutcome::Complete(Ok(ControlFlow::Continue(input))) => {
                *state = Some(S::start(input));
                StepOutcome::InProgress
            }
            StepOutcome::Complete(Ok(ControlFlow::Break(output))) => {
                StepOutcome::Complete(Ok(output))
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
        })
    }
}
