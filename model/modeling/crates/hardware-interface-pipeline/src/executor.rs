//! Policies for driving a [`crate::Stage`].

mod asynchronous;
mod immediate;
mod stepped;

pub use asynchronous::{AsyncExecutor, AsyncRunError};
pub use immediate::ImmediateExecutor;
pub use stepped::StepExecutor;

/// A non-terminal outcome produced by a synchronous executor.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a pipeline run outcome must be handled"]
pub enum RunOutcome<O, E, W> {
    /// One stage interaction committed and the pipeline remains runnable.
    Progressed,
    /// The pipeline transfers its owned wait state to the caller.
    Pending(W),
    /// The pipeline transfers its owned terminal success or error to the caller.
    Complete(Result<O, E>),
}

/// The typed result of advancing a pipeline through a synchronous executor.
pub type RunResult<O, E, W> = Result<RunOutcome<O, E, W>, RunError>;

/// A synchronous executor could not produce a pipeline outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a pipeline run error must be handled"]
pub enum RunError {
    /// The immediate executor consumed its configured transition budget.
    BudgetExhausted,
    /// The pipeline was already terminal before the executor call.
    AlreadyComplete,
}
