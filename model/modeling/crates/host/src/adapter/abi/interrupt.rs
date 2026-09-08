use crate::runtime::interrupt::PdmaNotificationEnvelope;
use hardware_interface_runtime::InterruptEnvelope;
use pdma_model_contract::semantic::{DeviceEpoch, IrqNotification, IrqVector};
use std::{error::Error, fmt};

/// Affine proof that a PDMA interrupt source reported readiness.
#[derive(Debug, PartialEq, Eq)]
pub struct PdmaInterruptReady(DeviceEpoch);

impl PdmaInterruptReady {
    /// Captures the device epoch of a non-consuming readiness observation.
    pub const fn new(epoch: DeviceEpoch) -> Self {
        Self(epoch)
    }

    /// Returns the epoch observed before consuming the interrupt output.
    pub const fn epoch(&self) -> DeviceEpoch {
        self.0
    }
}

/// One notification consumed from the hardware interrupt output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdmaInterruptNotification {
    notification: IrqNotification,
}

impl PdmaInterruptNotification {
    /// Wraps one notification consumed from a hardware interrupt output.
    pub const fn new(notification: IrqNotification) -> Self {
        Self { notification }
    }

    /// Returns the semantic notification captured by the exact claim.
    pub const fn notification(&self) -> IrqNotification {
        self.notification
    }
}

impl InterruptEnvelope for PdmaInterruptNotification {
    type Vector = IrqVector;

    fn vector(&self) -> Self::Vector {
        self.notification.vector()
    }
}

impl PdmaNotificationEnvelope for PdmaInterruptNotification {
    fn notification(&self) -> IrqNotification {
        self.notification
    }
}

/// Failure from observing or consuming the PDMA hardware interrupt output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaInterruptSourceError {
    /// The device interrupt source closed.
    Closed,
    /// Readiness or consumption belongs to another reset epoch.
    StaleEpoch {
        /// Required device epoch.
        expected: DeviceEpoch,
        /// Supplied capability epoch.
        actual: DeviceEpoch,
    },
    /// Hardware rejected its interrupt output state.
    InvalidState,
}

impl fmt::Display for PdmaInterruptSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("PDMA interrupt source closed"),
            Self::StaleEpoch { expected, actual } => write!(
                formatter,
                "stale interrupt epoch: expected {expected:?}, actual {actual:?}"
            ),
            Self::InvalidState => formatter.write_str("PDMA interrupt output state is invalid"),
        }
    }
}

impl Error for PdmaInterruptSourceError {}
