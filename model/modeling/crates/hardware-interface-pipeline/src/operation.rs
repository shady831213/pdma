use std::{convert::Infallible, marker::PhantomData};

use crate::{RunError, Stage, StepOutcome, StepResult};

/// One request's retained state, independent of hardware and any driving future.
pub struct Operation<S: Stage<C>, C> {
    state: Option<S::State>,
    completion: Option<Result<S::Output, S::Error>>,
    stage: PhantomData<fn(&mut S, &mut C)>,
}

impl<S: Stage<C>, C> Operation<S, C> {
    /// Creates a new request without touching hardware or context.
    pub fn new(input: S::Input) -> Self {
        Self {
            state: Some(S::start(input)),
            completion: None,
            stage: PhantomData,
        }
    }

    /// Drops retained request state; previously committed hardware effects remain.
    pub fn reset(&mut self) {
        self.state = None;
        self.completion = None;
    }

    /// Reports whether this operation has completed or was reset.
    pub const fn is_complete(&self) -> bool {
        self.state.is_none()
    }

    /// Advances this operation once; completed operations are inert.
    pub fn step(&mut self, hardware: &mut S, context: &mut C) -> StepResult<S::Output, S::Error> {
        match self.advance(hardware, context)? {
            StepOutcome::InProgress => Ok(StepOutcome::InProgress),
            StepOutcome::Complete(Ok(())) => self
                .take_completion()
                .map(StepOutcome::Complete)
                .ok_or(RunError::Exhausted),
            StepOutcome::Complete(Err(error)) => match error {},
        }
    }

    pub(crate) const fn take_completion(&mut self) -> Option<Result<S::Output, S::Error>> {
        self.completion.take()
    }

    pub(crate) fn advance(
        &mut self,
        hardware: &mut S,
        context: &mut C,
    ) -> StepResult<(), Infallible> {
        if self.completion.is_some() {
            return Ok(StepOutcome::Complete(Ok(())));
        }
        let Some(state) = self.state.as_mut() else {
            return Err(RunError::Exhausted);
        };
        match hardware.step(context, state) {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => {
                self.state = None;
                self.completion = Some(result);
                Ok(StepOutcome::Complete(Ok(())))
            }
            Err(error) => {
                self.reset();
                Err(error)
            }
        }
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
            .field("has_completion", &self.completion.is_some())
            .finish_non_exhaustive()
    }
}
