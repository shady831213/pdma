/// The complete observable result algebra for one pipeline step.
///
/// Wait, output, and error values cross the step boundary by ownership. A caller can therefore
/// retain or drop them without borrowing the stage, and dropping a wait value is an explicit
/// cancellation boundary.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a pipeline step outcome must be handled"]
pub enum StepOutcome<O, E, W> {
    /// One interaction committed and the stage advanced before returning.
    Progressed,
    /// The stage cannot currently advance and transfers its wait state to the caller.
    Pending(W),
    /// The operation reached its output or error and transfers that value to the caller.
    Complete(Result<O, E>),
    /// This operation was already terminal and performed no interaction.
    Exhausted,
}
