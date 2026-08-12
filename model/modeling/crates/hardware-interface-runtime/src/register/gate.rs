use super::{RegisterObservation, SharedRegisterObserver};
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
    next_reservation: u128,
    next_emission: u128,
    emitter_active: bool,
    waiting_emitters: usize,
    fault: Option<GateFault>,
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
                next_reservation: u128::from(next_sequence),
                next_emission: u128::from(next_sequence),
                emitter_active: false,
                waiting_emitters: 0,
                fault: None,
            }),
            changed: Condvar::new(),
        }))
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        let state = self.lock_state()?;
        Self::ensure_open(&state)?;
        u64::try_from(state.next_reservation).map_err(|_| ObservationGateError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 1,
        })
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(&self, count: usize) -> Result<ObservationReservation, ObservationGateError> {
        if count == 0 {
            return Err(ObservationGateError::InvalidReservationCount);
        }
        let mut state = self.lock_state()?;
        Self::ensure_open(&state)?;
        let count_u128 = count as u128;
        let final_sequence = state
            .next_reservation
            .checked_add(count_u128 - 1)
            .filter(|sequence| *sequence <= u128::from(u64::MAX))
            .ok_or_else(|| ObservationGateError::SequenceExhausted {
                next_sequence: u64::try_from(state.next_reservation).unwrap_or(u64::MAX),
                count,
            })?;
        let start = u64::try_from(state.next_reservation).map_err(|_| {
            ObservationGateError::SequenceExhausted {
                next_sequence: u64::MAX,
                count,
            }
        })?;
        state.next_reservation = final_sequence + 1;
        drop(state);
        Ok(ObservationReservation {
            gate: self.clone(),
            start,
            count,
            armed: true,
        })
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, GateState>, ObservationGateError> {
        self.0.state.lock().map_err(|poisoned| {
            let mut state = poisoned.into_inner();
            state
                .fault
                .get_or_insert(GateFault::SynchronizationPoisoned);
            drop(state);
            self.0.changed.notify_all();
            ObservationGateError::Faulted {
                reason: GateFault::SynchronizationPoisoned,
            }
        })
    }

    fn ensure_open(state: &GateState) -> Result<(), ObservationGateError> {
        state.fault.map_or(Ok(()), |reason| {
            Err(ObservationGateError::Faulted { reason })
        })
    }

    fn fault(&self, reason: GateFault) {
        match self.0.state.lock() {
            Ok(mut state) => {
                state.fault.get_or_insert(reason);
                state.emitter_active = false;
            }
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state
                    .fault
                    .get_or_insert(GateFault::SynchronizationPoisoned);
                state.emitter_active = false;
            }
        }
        self.0.changed.notify_all();
    }
}

/// Affine committed sequence range that must synchronously emit or fault its gate.
#[derive(Debug)]
pub struct ObservationReservation {
    gate: ObservationGate,
    start: u64,
    count: usize,
    armed: bool,
}

impl ObservationReservation {
    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.start
    }

    /// Emits the exact reserved range synchronously in sequence order.
    pub fn emit(
        mut self,
        observations: Vec<RegisterObservation>,
    ) -> Result<(), ObservationGateError> {
        if !self.matches(&observations) {
            self.armed = false;
            self.gate.fault(GateFault::InvalidEmission);
            return Err(ObservationGateError::Faulted {
                reason: GateFault::InvalidEmission,
            });
        }
        let mut state = self.gate.lock_state()?;
        loop {
            ObservationGate::ensure_open(&state)?;
            if !state.emitter_active && state.next_emission == u128::from(self.start) {
                state.emitter_active = true;
                break;
            }
            state.waiting_emitters += 1;
            self.gate.0.changed.notify_all();
            state = self.gate.0.changed.wait(state).map_err(|poisoned| {
                let mut poisoned_state = poisoned.into_inner();
                poisoned_state
                    .fault
                    .get_or_insert(GateFault::SynchronizationPoisoned);
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
        state.next_emission += self.count as u128;
        state.emitter_active = false;
        drop(state);
        self.gate.0.changed.notify_all();
        Ok(())
    }

    fn matches(&self, observations: &[RegisterObservation]) -> bool {
        observations.len() == self.count
            && observations.iter().enumerate().all(|(index, observation)| {
                u64::try_from(index)
                    .ok()
                    .and_then(|index| self.start.checked_add(index))
                    == Some(observation.sequence)
            })
    }
}

impl Drop for ObservationReservation {
    fn drop(&mut self) {
        if self.armed {
            self.armed = false;
            self.gate.fault(GateFault::ReservationDropped);
        }
    }
}

#[cfg(test)]
mod tests;
