use std::marker::PhantomData;

use crate::{RunOutcome, Stage, StepOutcome};

/// An operation has already completed or been reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlreadyComplete;

impl std::fmt::Display for AlreadyComplete {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("operation already complete")
    }
}

impl std::error::Error for AlreadyComplete {}

/// An operation outcome or a repeated-call protocol error.
pub type OperationResult<O, E, W> = Result<RunOutcome<O, E, W>, AlreadyComplete>;

/// One request's retained state, independent of hardware and any driving future.
pub struct Operation<S: Stage<C>, C> {
    state: Option<S::State>,
    stage: PhantomData<fn(&mut S, &mut C)>,
}

impl<S: Stage<C>, C> Operation<S, C> {
    /// Creates a new request without touching hardware or context.
    pub fn new(input: S::Input) -> Self {
        Self::from_state(S::start(input))
    }

    /// Resumes owned state without calling [`Stage::start`] or touching hardware.
    pub const fn from_state(state: S::State) -> Self {
        Self {
            state: Some(state),
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
    ///
    /// # Errors
    /// Returns [`AlreadyComplete`] without touching hardware after completion or reset.
    pub fn step(
        &mut self,
        hardware: &mut S,
        context: &mut C,
    ) -> OperationResult<S::Output, S::Error, S::Wait> {
        let state = self.state.take().ok_or(AlreadyComplete)?;
        Ok(match hardware.step(context, state) {
            StepOutcome::Progressed(state) => {
                self.state = Some(state);
                RunOutcome::Progressed
            }
            StepOutcome::Pending(state, wait) => {
                self.state = Some(state);
                RunOutcome::Pending(wait)
            }
            StepOutcome::Complete(result) => RunOutcome::Complete(result),
        })
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
