use crate::{Operation, Stage, executor::RunError, executor::RunResult};

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
        operation: &mut Operation<S, C>,
        context: &mut C,
    ) -> RunResult<S::Output, S::Error, S::Wait>
    where
        S: Stage<C>,
    {
        operation
            .step(stage, context)
            .map_err(|_| RunError::AlreadyComplete)
    }
}
