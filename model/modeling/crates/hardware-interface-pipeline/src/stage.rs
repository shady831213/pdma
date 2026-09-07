use crate::StepOutcome;

/// Reusable hardware with typed ports and separately owned operation state.
///
/// Hardware outlives each operation. Starting creates request state, never hardware.
/// Each step performs at most one bounded interaction and retains accepted work.
/// Domain faults needing downstream handling belong in Output; Error terminates
/// the operation. No context borrow escapes a step.
pub trait Stage<C> {
    /// Owned request accepted by this hardware.
    type Input;
    /// Owned result passed downstream.
    type Output;
    /// Structural failure terminating this operation.
    type Error;
    /// Owned notification key for a suspended operation.
    type Wait;
    /// Request state retained independently of hardware and the driving future.
    type State;

    /// Constructs request state without interacting with hardware or context.
    fn start(input: Self::Input) -> Self::State;

    /// Advances one operation through at most one bounded interaction.
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait>;
}

impl<C, S: Stage<C> + ?Sized> Stage<C> for &mut S {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    type Wait = S::Wait;
    type State = S::State;

    fn start(input: Self::Input) -> Self::State {
        S::start(input)
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        S::step(self, context, state)
    }
}
