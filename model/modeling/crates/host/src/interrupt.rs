//! Host interrupt surface.

mod pdma;
mod trace;

pub use pdma::{
    PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSource, PdmaInterruptSourceError,
    PdmaIrqDelivery, PdmaIrqHandler, PdmaIrqService, PdmaIrqServiceError,
};
pub use trace::PdmaInterruptTrace;

use crate::core::PortError;
pub use crate::driver::DriverIrqRecord;
pub use crate::irq_delivery::{
    HasHandlers, IrqDelivery, IrqDeliveryBootstrap, IrqDeliveryHandle, IrqDeliveryRunner,
    IrqDeliverySummary, IrqHandler, IrqHandlerFuture, IrqHandlerRegistry, NoHandler,
    StatefulIrqHandler, irq_handler, stateful_irq_handler,
};
use pdma_model_contract::IrqVector;
use std::{error::Error, fmt};

/// Failure from host interrupt registration or delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl From<crate::ModelError> for InterruptError {
    fn from(source: crate::ModelError) -> Self {
        match source {
            crate::ModelError::DuplicateIrqVector(vector) => Self::DuplicateVector(vector),
            crate::ModelError::UnregisteredIrqVector(vector) => Self::UnregisteredVector(vector),
            crate::ModelError::EmptyIrqHandlerRegistry => Self::EmptyRegistry,
            crate::ModelError::IrqDeliveryUnavailable => Self::DeliveryUnavailable,
            source => Self::Port(source.into()),
        }
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
        }
    }
}

impl Error for InterruptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Port(source) => Some(source),
            Self::DuplicateVector(_)
            | Self::UnregisteredVector(_)
            | Self::EmptyRegistry
            | Self::DeliveryUnavailable => None,
        }
    }
}
