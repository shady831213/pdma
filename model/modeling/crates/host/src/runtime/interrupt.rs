//! Host interrupt surface.

mod pdma;
mod trace;

pub use pdma::{
    PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService, PdmaIrqServiceError, PdmaNotificationEnvelope,
};
pub use trace::PdmaInterruptTrace;

pub use crate::semantic::DriverIrqRecord;
use crate::semantic::{DriverError, PortError};
use pdma_model_contract::semantic::IrqVector;
use std::{error::Error, fmt};

/// Failure from host interrupt registration or delivery.
#[derive(Debug, PartialEq, Eq)]
pub enum InterruptError {
    /// The vector already has a registered handler.
    DuplicateVector(IrqVector),
    /// The vector has no registered handler.
    UnregisteredVector(IrqVector),
    /// A frozen registry contains no handler.
    EmptyRegistry,
    /// The delivery task cannot accept its bootstrap state.
    DeliveryUnavailable,
    /// A device-facing interrupt operation failed.
    Port(PortError),
    /// A CPU-bound driver turn failed.
    Driver(DriverError),
}

/// Result returned by host interrupt registration or delivery.
pub type InterruptResult<T> = Result<T, InterruptError>;

impl From<PortError> for InterruptError {
    fn from(source: PortError) -> Self {
        Self::Port(source)
    }
}

impl From<DriverError> for InterruptError {
    fn from(source: DriverError) -> Self {
        Self::Driver(source)
    }
}

impl fmt::Display for InterruptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateVector(_) => {
                formatter.write_str("interrupt vector is already registered")
            }
            Self::UnregisteredVector(_) => {
                formatter.write_str("interrupt vector is not registered")
            }
            Self::EmptyRegistry => formatter.write_str("interrupt registry is empty"),
            Self::DeliveryUnavailable => formatter.write_str("interrupt delivery is unavailable"),
            Self::Port(_) => formatter.write_str("interrupt port operation failed"),
            Self::Driver(_) => formatter.write_str("interrupt driver operation failed"),
        }
    }
}

impl Error for InterruptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Port(source) => Some(source),
            Self::Driver(source) => Some(source),
            Self::DuplicateVector(_)
            | Self::UnregisteredVector(_)
            | Self::EmptyRegistry
            | Self::DeliveryUnavailable => None,
        }
    }
}
