use hardware_interface_schema::RegisterAccessError;
use pdma_model_hw_components::{control::ControlError, windows};
use std::fmt::{self, Display, Formatter};

/// Hardware access failure returned through the restricted Host bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError {
    /// Firmware has closed downstream host access.
    Closed,
    /// Host software has no permission for the requested management access.
    Denied,
    /// The complete transaction is outside the mapped downstream range.
    Unmapped,
    /// Generated status-register validation rejected the transaction.
    Register(RegisterAccessError),
    /// The independent control channel rejected a register transaction.
    Control(RegisterAccessError<ControlError>),
    /// The downstream hardware rejected the transaction.
    Downstream(pdma_model_dma::Error),
}

impl From<windows::AccessError<pdma_model_dma::Error>> for AccessError {
    fn from(error: windows::AccessError<pdma_model_dma::Error>) -> Self {
        match error {
            windows::AccessError::Closed => Self::Closed,
            windows::AccessError::Denied => Self::Denied,
            windows::AccessError::Unmapped => Self::Unmapped,
            windows::AccessError::Register(error) => Self::Register(error),
            windows::AccessError::Downstream(error) => Self::Downstream(error),
        }
    }
}

impl Display for AccessError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("downstream access is closed"),
            Self::Denied => formatter.write_str("host management access denied"),
            Self::Unmapped => formatter.write_str("transaction outside downstream window"),
            Self::Register(error) => Display::fmt(error, formatter),
            Self::Control(error) => Display::fmt(error, formatter),
            Self::Downstream(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for AccessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Register(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Downstream(error) => Some(error),
            Self::Closed | Self::Denied | Self::Unmapped => None,
        }
    }
}
