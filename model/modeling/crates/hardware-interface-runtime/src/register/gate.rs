use super::backend::ObservationGateError;
use super::{
    BlockingObservationBackend, BlockingObservationReservation, RegisterObservation,
    SharedRegisterObserver,
};

/// Clone-shared allocator and synchronous ordered emission gate.
#[derive(Clone, Debug)]
pub struct ObservationGate(pub(crate) BlockingObservationBackend);

impl ObservationGate {
    /// Creates a gate whose first reservation starts at sequence zero.
    pub fn new(observer: SharedRegisterObserver) -> Self {
        Self(BlockingObservationBackend::new(observer))
    }

    /// Creates a gate with an explicit first sequence.
    pub fn new_at(observer: SharedRegisterObserver, next_sequence: u64) -> Self {
        Self(BlockingObservationBackend::new_at(observer, next_sequence))
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        self.0.next_sequence()
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(&self, count: usize) -> Result<ObservationReservation, ObservationGateError> {
        self.0.reserve(count).map(ObservationReservation)
    }
}

/// Affine committed sequence range that must synchronously emit or fault its gate.
#[derive(Debug)]
pub struct ObservationReservation(BlockingObservationReservation);

impl ObservationReservation {
    /// Cancels an unused reservation only when no later range has been reserved.
    ///
    /// Callers must serialize reservation and cancellation; misuse faults the gate.
    pub fn cancel_latest(self) -> Result<(), ObservationGateError> {
        self.0.cancel_latest()
    }

    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.0.start_sequence()
    }

    /// Emits the exact reserved range synchronously in sequence order.
    pub fn emit(self, observations: Vec<RegisterObservation>) -> Result<(), ObservationGateError> {
        self.0.emit(observations)
    }
}

#[cfg(test)]
mod tests;
