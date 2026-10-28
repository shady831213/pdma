use std::num::NonZeroUsize;

use crate::{Operation, RunError, Stage, StepOutcome, StepResult};

/// An executor that advances a stage through a bounded number of steps.
#[derive(Clone, Copy, Debug)]
pub struct ImmediateExecutor {
    transition_budget: NonZeroUsize,
}

impl ImmediateExecutor {
    /// Creates an executor with a nonzero transition budget for every [`Self::run`] call.
    pub const fn new(transition_budget: NonZeroUsize) -> Self {
        Self { transition_budget }
    }

    /// Returns the nonzero number of stage steps permitted per run.
    pub const fn transition_budget(self) -> NonZeroUsize {
        self.transition_budget
    }

    /// Runs until completion or this call's transition budget is exhausted.
    pub fn run<C, S>(
        &self,
        stage: &mut S,
        operation: &mut Operation<S, C>,
        context: &mut C,
    ) -> StepResult<S::Output, S::Error>
    where
        S: Stage<C>,
    {
        for _ in 0..self.transition_budget.get() {
            match operation.step(stage, context)? {
                StepOutcome::InProgress => {}
                StepOutcome::Complete(result) => return Ok(StepOutcome::Complete(result)),
            }
        }
        Err(RunError::BudgetExhausted)
    }
}
