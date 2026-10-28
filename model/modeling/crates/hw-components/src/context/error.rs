use core::fmt::{self, Display, Formatter};
use std::collections::TryReserveError;

use hw_model_schema::RegisterBankError;

/// Failure to construct an instance-sized context table.
#[derive(Debug)]
pub enum ContextConfigError {
    /// The context capacity cannot be represented by the capacity register.
    InvalidCapacity,
    /// The generated table layout or its placement is invalid.
    Layout(RegisterBankError),
    /// Storage for context entries or channel failures could not be reserved.
    Allocation(TryReserveError),
}

impl Display for ContextConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity => formatter.write_str("invalid context-table capacity"),
            Self::Layout(error) => Display::fmt(error, formatter),
            Self::Allocation(error) => Display::fmt(error, formatter),
        }
    }
}

impl core::error::Error for ContextConfigError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidCapacity => None,
            Self::Layout(error) => Some(error),
            Self::Allocation(error) => Some(error),
        }
    }
}
