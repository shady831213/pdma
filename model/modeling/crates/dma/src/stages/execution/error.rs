use crate::{Completion, Error};
use std::fmt;

/// Execution rejection or an actual failed command ready for its completion tail.
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionError {
    /// A stopping command error, carrying the real response and destination.
    CommandFailed(Completion),
    /// Reset, halt, routing or other hardware rejection.
    Rejected(Error),
}

impl From<Error> for ExecutionError {
    fn from(error: Error) -> Self {
        Self::Rejected(error)
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "DMA execution: {self:?}")
    }
}
impl std::error::Error for ExecutionError {}

impl From<hardware_interface_pipeline::BranchError<Self>> for ExecutionError {
    fn from(error: hardware_interface_pipeline::BranchError<Self>) -> Self {
        use hardware_interface_pipeline::BranchError;
        match error {
            BranchError::Application(error) => error,
            BranchError::InvalidIndex { .. }
            | BranchError::DuplicateIndex { .. }
            | BranchError::Unavailable { .. }
            | BranchError::EmptyInput
            | BranchError::InputCount { .. }
            | BranchError::OutputCount { .. } => Error::Routing.into(),
        }
    }
}
