use std::num::NonZeroUsize;

use crate::{Stage, StepOutcome, executor::RunError, executor::RunOutcome, executor::RunResult};

/// A bounded executor that runs a stage through consecutive progressed transitions.
#[derive(Clone, Copy, Debug)]
pub struct ImmediateExecutor {
    transition_budget: NonZeroUsize,
}

impl ImmediateExecutor {
    /// Creates an executor with a nonzero transition budget for every [`Self::run`] call.
    pub const fn new(transition_budget: NonZeroUsize) -> Self {
        Self { transition_budget }
    }

    /// Returns the nonzero number of progressed transitions permitted per run.
    pub const fn transition_budget(self) -> NonZeroUsize {
        self.transition_budget
    }

    /// Runs until a non-progress outcome or this call's transition budget is exhausted.
    pub fn run<C, S>(
        &self,
        stage: &mut S,
        context: &mut C,
    ) -> RunResult<S::Output, S::Error, S::Wait>
    where
        S: Stage<C>,
    {
        for _ in 0..self.transition_budget.get() {
            match stage.step(context) {
                StepOutcome::Progressed => {}
                StepOutcome::Pending(wait) => return Ok(RunOutcome::Pending(wait)),
                StepOutcome::Complete(result) => return Ok(RunOutcome::Complete(result)),
                StepOutcome::Exhausted => return Err(RunError::Exhausted),
            }
        }
        Err(RunError::BudgetExhausted)
    }
}
