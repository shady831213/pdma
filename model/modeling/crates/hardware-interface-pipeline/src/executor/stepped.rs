use crate::{Stage, StepOutcome, executor::RunError, executor::RunOutcome, executor::RunResult};

/// A deterministic executor that performs exactly one stage call per [`Self::step`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StepExecutor;

impl StepExecutor {
    /// Creates a one-step executor.
    pub const fn new() -> Self {
        Self
    }

    /// Advances a stage exactly once and normalizes its observable outcome.
    pub fn step<C, S>(
        &self,
        stage: &mut S,
        context: &mut C,
    ) -> RunResult<S::Output, S::Error, S::Wait>
    where
        S: Stage<C>,
    {
        match stage.step(context) {
            StepOutcome::Progressed => Ok(RunOutcome::Progressed),
            StepOutcome::Pending(wait) => Ok(RunOutcome::Pending(wait)),
            StepOutcome::Complete(result) => Ok(RunOutcome::Complete(result)),
            StepOutcome::Exhausted => Err(RunError::Exhausted),
        }
    }
}
