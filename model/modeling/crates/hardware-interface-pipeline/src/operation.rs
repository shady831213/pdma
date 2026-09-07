use std::marker::PhantomData;

use crate::{Stage, StepOutcome};

/// One request's retained state, independent of hardware and any driving future.
pub struct Operation<S: Stage<C>, C> {
    state: Option<S::State>,
    stage: PhantomData<fn(&mut S, &mut C)>,
}

impl<S: Stage<C>, C> Operation<S, C> {
    /// Creates a new request without touching hardware or context.
    pub fn new(input: S::Input) -> Self {
        Self {
            state: Some(S::start(input)),
            stage: PhantomData,
        }
    }

    /// Drops retained request state; previously committed hardware effects remain.
    pub fn reset(&mut self) {
        self.state = None;
    }

    /// Reports whether this operation has completed or was reset.
    pub const fn is_complete(&self) -> bool {
        self.state.is_none()
    }

    /// Advances this operation once; completed operations are inert.
    pub fn step(
        &mut self,
        hardware: &mut S,
        context: &mut C,
    ) -> StepOutcome<S::Output, S::Error, S::Wait> {
        let Some(state) = self.state.as_mut() else {
            return StepOutcome::Exhausted;
        };
        let outcome = hardware.step(context, state);
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            self.reset();
        }
        outcome
    }
}

impl<S: Stage<C>, C> std::fmt::Debug for Operation<S, C>
where
    S::State: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operation")
            .field("state", &self.state)
            .finish()
    }
}
