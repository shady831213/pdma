use std::{convert::Infallible, marker::PhantomData};

use crate::{RunError, Stage, StepOutcome, StepResult};

/// Pending input and terminal receipt retained independently of a driving future.
pub struct Operation<S: Stage<C>, C> {
    input: Option<S::Input>,
    active: bool,
    completion: Option<Result<S::Output, S::Error>>,
    run_error: Option<RunError>,
    stage: PhantomData<fn(&mut S, &mut C)>,
}

impl<S: Stage<C>, C> Operation<S, C> {
    /// Creates a new request without touching hardware or context.
    pub fn new(input: S::Input) -> Self {
        Self {
            input: Some(input),
            active: true,
            completion: None,
            run_error: None,
            stage: PhantomData,
        }
    }

    /// Reports whether hardware completed this operation.
    pub const fn is_complete(&self) -> bool {
        !self.active
    }

    /// Advances this operation once; completed operations are inert.
    pub fn step(&mut self, hardware: &mut S, context: &mut C) -> StepResult<S::Output, S::Error> {
        match self.advance(hardware, context)? {
            StepOutcome::InProgress => Ok(StepOutcome::InProgress),
            StepOutcome::Complete(Ok(())) => {
                if let Some(error) = self.take_run_error() {
                    return Err(error);
                }
                self.take_completion()
                    .map(StepOutcome::Complete)
                    .ok_or(RunError::Exhausted)
            }
            StepOutcome::Complete(Err(error)) => match error {},
        }
    }

    pub(crate) const fn take_completion(&mut self) -> Option<Result<S::Output, S::Error>> {
        self.completion.take()
    }

    pub(crate) const fn take_run_error(&mut self) -> Option<RunError> {
        self.run_error.take()
    }

    pub(crate) fn advance(
        &mut self,
        hardware: &mut S,
        context: &mut C,
    ) -> StepResult<(), Infallible> {
        if self.completion.is_some() || self.run_error.is_some() {
            return Ok(StepOutcome::Complete(Ok(())));
        }
        if !self.active {
            return Err(RunError::Exhausted);
        }
        match hardware.step(context, &mut self.input) {
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Ok(StepOutcome::Complete(result)) => {
                self.input = None;
                self.active = false;
                self.completion = Some(result);
                Ok(StepOutcome::Complete(Ok(())))
            }
            Err(error) => {
                self.run_error = Some(error);
                Ok(StepOutcome::Complete(Ok(())))
            }
        }
    }
}

impl<S: Stage<C>, C> std::fmt::Debug for Operation<S, C>
where
    S::Input: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operation")
            .field("input", &self.input)
            .field("active", &self.active)
            .field("has_completion", &self.completion.is_some())
            .finish_non_exhaustive()
    }
}
