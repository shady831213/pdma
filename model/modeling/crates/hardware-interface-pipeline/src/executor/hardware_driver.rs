use std::convert::Infallible;

use crate::{Operation, RunError, Stage, StepOutcome, StepResult};

/// External driving state for successive finite unit-input, unit-output services.
///
/// This is not a stage: only an external driving step starts a fresh invocation
/// after the previous one succeeds. Unfinished invocation state and terminal
/// errors survive cancellation of an asynchronous driving future.
///
/// A driver cannot be nested inside a pipeline:
///
/// ```compile_fail
/// use hardware_interface_pipeline::{HardwareDriver, Stage};
/// fn nested<S: Stage<(), Input = (), Output = ()>>() {
///     fn stage<T: Stage<()>>() {}
///     stage::<HardwareDriver<S, ()>>();
/// }
/// ```
///
/// Services cannot accept arbitrary inputs:
///
/// ```compile_fail
/// use hardware_interface_pipeline::{HardwareDriver, Stage};
/// fn arbitrary_input<S: Stage<(), Input = u8, Output = ()>>() {
///     let _driver = HardwareDriver::<S, ()>::new();
/// }
/// ```
pub struct HardwareDriver<S: Stage<C, Input = (), Output = ()>, C> {
    state: DriverState<S, C>,
}

enum DriverState<S: Stage<C, Input = (), Output = ()>, C> {
    Ready,
    Running(Operation<S, C>),
    Complete(Option<Result<S::Error, RunError>>),
}

impl<S: Stage<C, Input = (), Output = ()>, C> HardwareDriver<S, C> {
    /// Creates a driver without starting an invocation or touching hardware.
    pub const fn new() -> Self {
        Self {
            state: DriverState::Ready,
        }
    }

    /// Reports whether a stage failure or call error stopped this driver.
    pub const fn is_complete(&self) -> bool {
        matches!(self.state, DriverState::Complete(_))
    }

    /// Performs one bounded hardware step, starting a service when needed.
    ///
    /// Successful service completion returns `InProgress`; a new invocation
    /// starts only on a later call. Stage errors and call errors stop driving,
    /// are delivered once, and leave subsequent calls exhausted.
    pub fn step(&mut self, hardware: &mut S, context: &mut C) -> StepResult<(), S::Error> {
        match self.advance(hardware, context)? {
            StepOutcome::InProgress => Ok(StepOutcome::InProgress),
            StepOutcome::Complete(Ok(())) => match self.take_failure() {
                Some(Ok(error)) => Ok(StepOutcome::Complete(Err(error))),
                Some(Err(error)) => Err(error),
                None => Err(RunError::Exhausted),
            },
            StepOutcome::Complete(Err(error)) => match error {},
        }
    }

    pub(super) const fn take_failure(&mut self) -> Option<Result<S::Error, RunError>> {
        match &mut self.state {
            DriverState::Complete(failure) => failure.take(),
            DriverState::Ready | DriverState::Running(_) => None,
        }
    }

    pub(super) fn advance(
        &mut self,
        hardware: &mut S,
        context: &mut C,
    ) -> StepResult<(), Infallible> {
        let mut operation = match std::mem::replace(&mut self.state, DriverState::Complete(None)) {
            DriverState::Ready => Operation::new(()),
            DriverState::Running(operation) => operation,
            DriverState::Complete(failure) => {
                let pending = failure.is_some();
                self.state = DriverState::Complete(failure);
                return if pending {
                    Ok(StepOutcome::Complete(Ok(())))
                } else {
                    Err(RunError::Exhausted)
                };
            }
        };
        match operation.step(hardware, context) {
            Ok(StepOutcome::InProgress) => {
                self.state = DriverState::Running(operation);
                Ok(StepOutcome::InProgress)
            }
            Ok(StepOutcome::Complete(Ok(()))) => {
                self.state = DriverState::Ready;
                Ok(StepOutcome::InProgress)
            }
            Ok(StepOutcome::Complete(Err(error))) => {
                self.state = DriverState::Complete(Some(Ok(error)));
                Ok(StepOutcome::Complete(Ok(())))
            }
            Err(error) => {
                self.state = DriverState::Complete(Some(Err(error)));
                Ok(StepOutcome::Complete(Ok(())))
            }
        }
    }
}

impl<S: Stage<C, Input = (), Output = ()>, C> Default for HardwareDriver<S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Stage<C, Input = (), Output = ()>, C> std::fmt::Debug for HardwareDriver<S, C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HardwareDriver")
            .field("is_complete", &self.is_complete())
            .finish_non_exhaustive()
    }
}
