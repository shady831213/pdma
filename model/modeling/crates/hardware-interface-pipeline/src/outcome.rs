/// The complete observable result algebra for one pipeline step.
///
/// Nonterminal outcomes return the live continuation by ownership; terminal outcomes
/// return only output or error. Wait values describe notification conditions, not
/// operation ownership. Dropping a wait value does not cancel accepted hardware work.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a pipeline step outcome must be handled"]
pub enum StepOutcome<S, O, E, W> {
    /// The operation advanced, possibly by an internal transition, and remains runnable.
    Progressed(S),
    /// Further progress needs a wait; retain this continuation and any committed effects.
    Pending(S, W),
    /// The operation reached its output or error and transfers that value to the caller.
    Complete(Result<O, E>),
}
