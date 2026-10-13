//! Policies for driving a [`crate::Stage`].

mod asynchronous;
mod hardware_driver;
mod immediate;
mod stepped;

pub use asynchronous::{AsyncExecutor, AsyncRunError};
pub use hardware_driver::HardwareDriver;
pub use immediate::ImmediateExecutor;
pub use stepped::StepExecutor;

/// A pipeline driving call could not produce a stage outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a pipeline run error must be handled"]
pub enum RunError {
    /// The immediate executor consumed its configured transition budget.
    BudgetExhausted,
    /// The pipeline was already terminal before the executor call.
    Exhausted,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::BudgetExhausted => "pipeline transition budget exhausted",
            Self::Exhausted => "pipeline operation already completed",
        })
    }
}

impl std::error::Error for RunError {}
