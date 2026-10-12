/// The value-flow outcome of one bounded hardware step.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a pipeline step outcome must be handled"]
pub enum StepOutcome<O, E> {
    /// The invocation retains unfinished work for another step.
    InProgress,
    /// Transfers the terminal result; no unfinished invocation work remains.
    Complete(Result<O, E>),
}

/// A step outcome or a violation of the driving call contract.
pub type StepResult<O, E> = Result<StepOutcome<O, E>, crate::RunError>;
