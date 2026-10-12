use crate::StepResult;

/// Reusable hardware with typed ports and separately owned operation state.
///
/// Hardware outlives each operation. Starting creates request state, never hardware.
/// Each step performs bounded work. `InProgress` retains the invocation without
/// promising forward progress. `Complete` transfers the result and releases all
/// invocation intermediates; persistent hardware configuration remains reusable.
/// Domain faults needing downstream handling belong in Output; Error terminates
/// the operation. No context borrow escapes a step.
pub trait Stage<C> {
    /// Owned request accepted by this hardware.
    type Input;
    /// Owned result passed downstream.
    type Output;
    /// Structural failure terminating this operation.
    type Error;
    /// Request state retained independently of hardware and the driving future.
    type State;

    /// Constructs request state without interacting with hardware or context.
    fn start(input: Self::Input) -> Self::State;

    /// Advances one invocation through bounded hardware work.
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error>;
}

impl<C, S: Stage<C> + ?Sized> Stage<C> for &mut S {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    type State = S::State;

    fn start(input: Self::Input) -> Self::State {
        S::start(input)
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        S::step(self, context, state)
    }
}
