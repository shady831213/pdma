use crate::StepResult;

/// Reusable hardware with typed ports and stage-owned transient state.
///
/// Taking input accepts it; busy stages leave new offers untouched through completion.
/// `None` offers no input; `Some(None)` is an actual optional domain input.
/// An idle stage with no offer is inert. Completion leaves transient storage reusable.
/// Each step performs bounded work. `InProgress` retains the invocation without
/// promising forward progress. `Complete` transfers the result and releases all
/// invocation intermediates; persistent hardware configuration remains reusable.
/// Domain faults needing downstream handling belong in Output; Error terminates
/// the operation. A raw `RunError` rejects driving without terminating accepted
/// work; the stage retains its continuation for later steps. No context borrow escapes a step.
pub trait Stage<C>: Sized {
    /// Owned request accepted by this hardware.
    type Input;
    /// Owned result passed downstream.
    type Output;
    /// Structural failure terminating this operation.
    type Error;
    /// Advances one invocation through bounded hardware work.
    ///
    /// # Errors
    /// The outer [`crate::RunError`] is a driving failure and preserves the
    /// accepted continuation. An inner `Complete(Err(error))` terminates the
    /// invocation with the stage's domain error. Callers must distinguish these
    /// layers before deciding whether to retry or release application resources.
    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error>;
}

impl<C, S: Stage<C>> Stage<C> for &mut S {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;

    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        S::step(self, context, input)
    }
}
