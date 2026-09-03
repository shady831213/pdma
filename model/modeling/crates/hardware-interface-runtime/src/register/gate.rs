use super::{
    ObservationOrder, ObservationOrderError, ObservationReservationReceipt, RegisterObservation,
    SharedRegisterObserver,
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// Permanent reason an ordered observation gate stopped accepting work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateFault {
    /// A committed reservation was dropped without emission.
    ReservationDropped,
    /// Emitted observations did not exactly match their reservation.
    InvalidEmission,
    /// The observer panicked during its synchronous callback.
    ObserverPanicked,
    /// Gate synchronization was poisoned by an interrupted critical section.
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
    /// The gate is terminally closed.
    Faulted {
        /// First terminal failure observed by the gate.
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
            Self::Faulted { reason } => write!(formatter, "observation gate faulted: {reason:?}"),
        }
    }
}

impl Error for ObservationGateError {}

#[derive(Debug)]
struct GateState {
    order: ObservationOrder,
    emitter_active: bool,
    waiting_emitters: usize,
}

#[derive(Debug)]
struct GateInner {
    observer: SharedRegisterObserver,
    state: Mutex<GateState>,
    changed: Condvar,
}

/// Clone-shared allocator and synchronous ordered emission gate.
#[derive(Clone, Debug)]
pub struct ObservationGate(Arc<GateInner>);

impl ObservationGate {
    /// Creates a gate whose first reservation starts at sequence zero.
    pub fn new(observer: SharedRegisterObserver) -> Self {
        Self::new_at(observer, 0)
    }

    /// Creates a gate with an explicit first sequence.
    pub fn new_at(observer: SharedRegisterObserver, next_sequence: u64) -> Self {
        Self(Arc::new(GateInner {
            observer,
            state: Mutex::new(GateState {
                order: ObservationOrder::new_at(next_sequence),
                emitter_active: false,
                waiting_emitters: 0,
            }),
            changed: Condvar::new(),
        }))
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        let state = self.lock_state()?;
        state.order.next_sequence().map_err(Self::order_error)
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(&self, count: usize) -> Result<ObservationReservation, ObservationGateError> {
        if count == 0 {
            return Err(ObservationGateError::InvalidReservationCount);
        }
        let mut state = self.lock_state()?;
        let receipt = state.order.reserve(count).map_err(Self::order_error)?;
        drop(state);
        Ok(ObservationReservation {
            gate: self.clone(),
            receipt,
            armed: true,
        })
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, GateState>, ObservationGateError> {
        self.0.state.lock().map_err(|poisoned| {
            let mut state = poisoned.into_inner();
            state.order.fault(GateFault::SynchronizationPoisoned);
            drop(state);
            self.0.changed.notify_all();
            ObservationGateError::Faulted {
                reason: GateFault::SynchronizationPoisoned,
            }
        })
    }

    fn fault(&self, reason: GateFault) {
        match self.0.state.lock() {
            Ok(mut state) => {
                state.order.fault(reason);
                state.emitter_active = false;
            }
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.order.fault(GateFault::SynchronizationPoisoned);
                state.emitter_active = false;
            }
        }
        self.0.changed.notify_all();
    }

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
}

/// Affine committed sequence range that must synchronously emit or fault its gate.
#[derive(Debug)]
pub struct ObservationReservation {
    gate: ObservationGate,
    receipt: ObservationReservationReceipt,
    armed: bool,
}

impl ObservationReservation {
    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.receipt.start_sequence()
    }

    /// Emits the exact reserved range synchronously in sequence order.
    pub fn emit(
        mut self,
        observations: Vec<RegisterObservation>,
    ) -> Result<(), ObservationGateError> {
        let mut state = self.gate.lock_state()?;
        let mut emission =
            match state
                .order
                .validate_with(&mut self.receipt, observations.len(), |index| {
                    observations
                        .get(index)
                        .map(|observation| observation.sequence)
                }) {
                Ok(emission) => emission,
                Err(error) => {
                    self.armed = false;
                    drop(state);
                    self.gate.0.changed.notify_all();
                    return Err(ObservationGate::order_error(error));
                }
            };
        self.armed = false;
        loop {
            let next_emission = state
                .order
                .next_emission_sequence()
                .map_err(ObservationGate::order_error)?;
            if !state.emitter_active && next_emission == self.start_sequence() {
                state.emitter_active = true;
                break;
            }
            state.waiting_emitters += 1;
            self.gate.0.changed.notify_all();
            state = self.gate.0.changed.wait(state).map_err(|poisoned| {
                let mut poisoned_state = poisoned.into_inner();
                poisoned_state
                    .order
                    .fault(GateFault::SynchronizationPoisoned);
                poisoned_state.emitter_active = false;
                drop(poisoned_state);
                self.gate.0.changed.notify_all();
                ObservationGateError::Faulted {
                    reason: GateFault::SynchronizationPoisoned,
                }
            })?;
            state.waiting_emitters -= 1;
        }
        self.armed = false;
        drop(state);
        for observation in observations {
            if catch_unwind(AssertUnwindSafe(|| {
                self.gate.0.observer.observe(observation);
            }))
            .is_err()
            {
                self.gate.fault(GateFault::ObserverPanicked);
                return Err(ObservationGateError::Faulted {
                    reason: GateFault::ObserverPanicked,
                });
            }
        }
        let mut state = self.gate.lock_state()?;
        state
            .order
            .advance(&mut emission)
            .map_err(ObservationGate::order_error)?;
        state.emitter_active = false;
        drop(state);
        self.gate.0.changed.notify_all();
        Ok(())
    }
}

impl Drop for ObservationReservation {
    fn drop(&mut self) {
        if self.armed {
            self.armed = false;
            match self.gate.0.state.lock() {
                Ok(mut state) => {
                    let _ = state.order.abandon(&mut self.receipt);
                    state.emitter_active = false;
                }
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.order.fault(GateFault::SynchronizationPoisoned);
                    state.emitter_active = false;
                }
            }
            self.gate.0.changed.notify_all();
        }
    }
}

#[cfg(test)]
mod tests;
