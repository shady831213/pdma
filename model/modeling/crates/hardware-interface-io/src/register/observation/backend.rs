//! Concrete synchronous observation policies.

mod blocking;
mod local;

pub use blocking::{ObservationGate, ObservationReservation};
pub use local::{LocalObservationGate, LocalObservationReservation};

use super::ObservationOrderError;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Permanent reason an ordered observation policy stopped accepting work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateFault {
    /// A committed reservation was dropped without emission.
    ReservationDropped,
    /// Emitted observations did not exactly match their reservation.
    InvalidEmission,
    /// The observer panicked during its synchronous callback.
    ObserverPanicked,
    /// Backend synchronization was poisoned by an interrupted critical section.
    SynchronizationPoisoned,
}

/// Failure to reserve or emit ordered committed observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationGateError {
    /// A reservation must contain at least one observation.
    InvalidReservationCount,
    /// The requested inclusive range exceeds `u64::MAX`.
    SequenceExhausted {
        /// First sequence that would have been reserved.
        next_sequence: u64,
        /// Requested contiguous sequence count.
        count: usize,
    },
    /// The observation policy is terminally closed.
    Faulted {
        /// First terminal failure observed by the policy.
        reason: GateFault,
    },
}

impl Display for ObservationGateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReservationCount => {
                formatter.write_str("reservation count must be nonzero")
            }
            Self::SequenceExhausted {
                next_sequence,
                count,
            } => write!(
                formatter,
                "reservation of {count} observations from sequence {next_sequence} exceeds u64"
            ),
            Self::Faulted { reason } => {
                write!(formatter, "observation gate faulted: {reason:?}")
            }
        }
    }
}

impl Error for ObservationGateError {}

const fn order_error(error: ObservationOrderError) -> ObservationGateError {
    match error {
        ObservationOrderError::InvalidReservationCount => {
            ObservationGateError::InvalidReservationCount
        }
        ObservationOrderError::SequenceExhausted {
            next_sequence,
            count,
        } => ObservationGateError::SequenceExhausted {
            next_sequence,
            count,
        },
        ObservationOrderError::EmissionOutOfOrder { .. } => ObservationGateError::Faulted {
            reason: GateFault::InvalidEmission,
        },
        ObservationOrderError::Faulted { reason } => ObservationGateError::Faulted { reason },
    }
}

const fn storage_error() -> ObservationGateError {
    ObservationGateError::Faulted {
        reason: GateFault::SynchronizationPoisoned,
    }
}
