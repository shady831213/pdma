use crate::StepOutcome;

/// A deterministic pipeline stage driven through explicit, bounded steps.
///
/// Each call to [`Stage::step`] performs at most one externally meaningful interaction. Before
/// returning [`StepOutcome::Progressed`], the implementation must advance its internal state so
/// the interaction cannot be repeated on the next call.
///
/// [`StepOutcome::Pending`] transfers owned wait state and marks a cancellation boundary: dropping
/// that value must not undo an interaction that already committed. Output and error values are
/// likewise transferred by ownership through [`StepOutcome::Complete`]. After completion, every
/// later call must return [`StepOutcome::Exhausted`] without performing an interaction.
pub trait Stage<C> {
    /// The owned successful terminal value.
    type Output;

    /// The owned failed terminal value.
    type Error;

    /// The owned state describing why the stage is pending.
    type Wait;

    /// Attempts at most one interaction and reports the stage's observable boundary.
    fn step(&mut self, context: &mut C) -> StepOutcome<Self::Output, Self::Error, Self::Wait>;
}
