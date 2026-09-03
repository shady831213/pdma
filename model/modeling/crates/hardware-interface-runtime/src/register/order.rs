use super::GateFault;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Failure to reserve, validate, or advance pure observation order state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationOrderError {
    /// A reservation must contain at least one observation.
    InvalidReservationCount,
    /// The requested inclusive range exceeds `u64::MAX`.
    SequenceExhausted {
        /// First sequence that would have been reserved.
        next_sequence: u64,
        /// Requested contiguous sequence count.
        count: usize,
    },
    /// A valid later emission cannot advance before its predecessor.
    EmissionOutOfOrder {
        /// Sequence currently eligible to advance.
        expected_sequence: u64,
        /// First sequence owned by the attempted emission.
        actual_sequence: u64,
    },
    /// The order is terminally closed.
    Faulted {
        /// First terminal failure observed by the order.
        reason: GateFault,
    },
}

impl Display for ObservationOrderError {
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
            Self::EmissionOutOfOrder {
                expected_sequence,
                actual_sequence,
            } => write!(
                formatter,
                "emission starts at sequence {actual_sequence}, expected {expected_sequence}"
            ),
            Self::Faulted { reason } => write!(formatter, "observation order faulted: {reason:?}"),
        }
    }
}

impl Error for ObservationOrderError {}

/// Affine ownership receipt for one committed contiguous sequence range.
///
/// ```compile_fail
/// use hardware_interface_runtime::ObservationOrder;
/// let mut order = ObservationOrder::new();
/// let receipt = order.reserve(1).unwrap();
/// let duplicate = receipt.clone();
/// drop((receipt, duplicate));
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ObservationReservationReceipt {
    start: u64,
    count: usize,
    active: bool,
}

impl ObservationReservationReceipt {
    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.start
    }
}

/// Affine proof that an exact reserved range is ready for ordered advancement.
///
/// ```compile_fail
/// use hardware_interface_runtime::ObservationOrder;
/// let mut order = ObservationOrder::new();
/// let mut reservation = order.reserve(1).unwrap();
/// let receipt = order.validate(&mut reservation, &[0]).unwrap();
/// let duplicate = receipt.clone();
/// drop((receipt, duplicate));
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ObservationEmissionReceipt {
    start: u64,
    count: usize,
    active: bool,
}

/// Pure allocation, validation, advancement, and permanent-fault state.
#[derive(Debug)]
pub struct ObservationOrder {
    next_reservation: u128,
    next_emission: u128,
    fault: Option<GateFault>,
}

impl Default for ObservationOrder {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationOrder {
    /// Creates an order whose first reservation starts at sequence zero.
    pub const fn new() -> Self {
        Self::new_at(0)
    }

    /// Creates an order with an explicit first sequence.
    pub const fn new_at(next_sequence: u64) -> Self {
        Self {
            next_reservation: next_sequence as u128,
            next_emission: next_sequence as u128,
            fault: None,
        }
    }

    /// Returns the next sequence allocated by a successful reservation.
    pub fn next_sequence(&self) -> Result<u64, ObservationOrderError> {
        self.ensure_open()?;
        u64::try_from(self.next_reservation).map_err(|_| ObservationOrderError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 1,
        })
    }

    /// Returns the first sequence not yet advanced.
    pub fn next_emission_sequence(&self) -> Result<u64, ObservationOrderError> {
        self.ensure_open()?;
        u64::try_from(self.next_emission).map_err(|_| ObservationOrderError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 1,
        })
    }

    /// Reserves a nonempty contiguous range and returns its sole ownership receipt.
    pub fn reserve(
        &mut self,
        count: usize,
    ) -> Result<ObservationReservationReceipt, ObservationOrderError> {
        if count == 0 {
            return Err(ObservationOrderError::InvalidReservationCount);
        }
        self.ensure_open()?;
        let count_u128 = count as u128;
        let final_sequence = self
            .next_reservation
            .checked_add(count_u128 - 1)
            .filter(|sequence| *sequence <= u128::from(u64::MAX))
            .ok_or_else(|| ObservationOrderError::SequenceExhausted {
                next_sequence: u64::try_from(self.next_reservation).unwrap_or(u64::MAX),
                count,
            })?;
        let start = u64::try_from(self.next_reservation).map_err(|_| {
            ObservationOrderError::SequenceExhausted {
                next_sequence: u64::MAX,
                count,
            }
        })?;
        self.next_reservation = final_sequence + 1;
        Ok(ObservationReservationReceipt {
            start,
            count,
            active: true,
        })
    }

    /// Consumes a reservation into an emission receipt after exact sequence validation.
    pub fn validate(
        &mut self,
        reservation: &mut ObservationReservationReceipt,
        sequences: &[u64],
    ) -> Result<ObservationEmissionReceipt, ObservationOrderError> {
        self.validate_with(reservation, sequences.len(), |index| {
            sequences.get(index).copied()
        })
    }

    pub(super) fn validate_with(
        &mut self,
        reservation: &mut ObservationReservationReceipt,
        count: usize,
        mut sequence_at: impl FnMut(usize) -> Option<u64>,
    ) -> Result<ObservationEmissionReceipt, ObservationOrderError> {
        self.ensure_open()?;
        let matches = reservation.active
            && count == reservation.count
            && (0..count).all(|index| {
                u64::try_from(index)
                    .ok()
                    .and_then(|index| reservation.start.checked_add(index))
                    == sequence_at(index)
            });
        if !matches {
            self.fault(GateFault::InvalidEmission);
            return Err(self.fault_error());
        }
        reservation.active = false;
        Ok(ObservationEmissionReceipt {
            start: reservation.start,
            count: reservation.count,
            active: true,
        })
    }

    /// Advances one validated range exactly once at its ordering linearization point.
    pub fn advance(
        &mut self,
        emission: &mut ObservationEmissionReceipt,
    ) -> Result<(), ObservationOrderError> {
        self.ensure_open()?;
        if !emission.active {
            self.fault(GateFault::InvalidEmission);
            return Err(self.fault_error());
        }
        if self.next_emission != u128::from(emission.start) {
            return Err(ObservationOrderError::EmissionOutOfOrder {
                expected_sequence: u64::try_from(self.next_emission).unwrap_or(u64::MAX),
                actual_sequence: emission.start,
            });
        }
        self.next_emission += emission.count as u128;
        emission.active = false;
        Ok(())
    }

    /// Permanently faults an active reservation that its owner abandons.
    pub fn abandon(
        &mut self,
        reservation: &mut ObservationReservationReceipt,
    ) -> Result<(), ObservationOrderError> {
        self.ensure_open()?;
        if !reservation.active {
            self.fault(GateFault::InvalidEmission);
            return Err(self.fault_error());
        }
        reservation.active = false;
        self.fault(GateFault::ReservationDropped);
        Ok(())
    }

    /// Records the first permanent failure; later failures cannot replace it.
    pub fn fault(&mut self, reason: GateFault) {
        self.fault.get_or_insert(reason);
    }

    fn ensure_open(&self) -> Result<(), ObservationOrderError> {
        self.fault.map_or(Ok(()), |reason| {
            Err(ObservationOrderError::Faulted { reason })
        })
    }

    fn fault_error(&self) -> ObservationOrderError {
        ObservationOrderError::Faulted {
            reason: self.fault.unwrap_or(GateFault::InvalidEmission),
        }
    }
}

/// Runtime-neutral policy for linearized access to one [`ObservationOrder`].
///
/// Each closure is one synchronization-policy-defined linearization point. The
/// higher-ranked lifetime prevents the order or a guard borrowing it from
/// escaping. Implementations choose storage and synchronization without a
/// trait-object default or a prescribed lock primitive.
pub trait ObservationBackend {
    /// Backend-specific access failure.
    type Error;

    /// Inspects the order during one linearized operation.
    fn inspect<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order ObservationOrder) -> R,
    ) -> Result<R, Self::Error>;

    /// Mutates the order during one linearized operation.
    fn mutate<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order mut ObservationOrder) -> R,
    ) -> Result<R, Self::Error>;
}
