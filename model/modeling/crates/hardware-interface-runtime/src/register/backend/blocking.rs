use crate::MutexStorage;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, MutexGuard, OnceLock};

use super::{ObservationGateError, order_error};
use crate::register::{
    GateFault, ObservationOrder, ObservationReservationReceipt, RegisterObservation,
    SharedRegisterObserver,
};

mod access;

#[derive(Debug)]
pub(in crate::register) struct BlockingState {
    order: ObservationOrder,
    emitter_active: bool,
    pub(in crate::register) waiting_emitters: usize,
}

/// Clone-shared cross-thread policy with ordered blocking emission.
#[derive(Clone, Debug)]
pub struct BlockingObservationBackend {
    observer: SharedRegisterObserver,
    pub(in crate::register) state: MutexStorage<BlockingState>,
    pub(in crate::register) changed: Arc<Condvar>,
    deferred_fault: Arc<OnceLock<GateFault>>,
}

impl BlockingObservationBackend {
    /// Creates a blocking policy whose first reservation starts at sequence zero.
    pub fn new(observer: SharedRegisterObserver) -> Self {
        Self::new_at(observer, 0)
    }

    /// Creates a blocking policy with an explicit first sequence.
    pub fn new_at(observer: SharedRegisterObserver, next_sequence: u64) -> Self {
        Self {
            observer,
            state: MutexStorage::new(BlockingState {
                order: ObservationOrder::new_at(next_sequence),
                emitter_active: false,
                waiting_emitters: 0,
            }),
            changed: Arc::new(Condvar::new()),
            deferred_fault: Arc::new(OnceLock::new()),
        }
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        self.lock_state()?
            .order
            .next_sequence()
            .map_err(order_error)
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(
        &self,
        count: usize,
    ) -> Result<BlockingObservationReservation, ObservationGateError> {
        let receipt = self
            .lock_state()?
            .order
            .reserve(count)
            .map_err(order_error)?;
        Ok(BlockingObservationReservation {
            backend: self.clone(),
            receipt,
            armed: true,
        })
    }
}

/// Affine contiguous range reserved from a [`BlockingObservationBackend`].
#[derive(Debug)]
pub struct BlockingObservationReservation {
    backend: BlockingObservationBackend,
    receipt: ObservationReservationReceipt,
    armed: bool,
}

impl BlockingObservationReservation {
    /// Cancels an unused latest reservation; a later reservation makes cancellation invalid.
    pub fn cancel_latest(mut self) -> Result<(), ObservationGateError> {
        let result = self
            .backend
            .lock_state()?
            .order
            .cancel_latest(&mut self.receipt)
            .map_err(order_error);
        self.armed = false;
        self.backend.changed.notify_all();
        result
    }

    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.receipt.start_sequence()
    }

    /// Blocks until this exact range can emit, then completes every callback.
    pub fn emit(
        mut self,
        observations: Vec<RegisterObservation>,
    ) -> Result<(), ObservationGateError> {
        let mut state = self.backend.lock_state()?;
        let mut emission =
            match state
                .order
                .validate_with(&mut self.receipt, observations.len(), |index| {
                    observations.get(index).map(|item| item.sequence)
                }) {
                Ok(emission) => emission,
                Err(error) => {
                    self.armed = false;
                    drop(state);
                    self.backend.changed.notify_all();
                    return Err(order_error(error));
                }
            };
        self.armed = false;
        state = self.wait_for_turn(state)?;
        drop(state);
        for observation in observations {
            if catch_unwind(AssertUnwindSafe(|| {
                self.backend.observer.observe(observation);
            }))
            .is_err()
            {
                self.backend.fault(GateFault::ObserverPanicked);
                return Err(ObservationGateError::Faulted {
                    reason: GateFault::ObserverPanicked,
                });
            }
        }
        let mut state = self.backend.lock_state()?;
        state.order.advance(&mut emission).map_err(order_error)?;
        state.emitter_active = false;
        drop(state);
        self.backend.changed.notify_all();
        Ok(())
    }

    fn wait_for_turn<'state>(
        &self,
        mut state: MutexGuard<'state, BlockingState>,
    ) -> Result<MutexGuard<'state, BlockingState>, ObservationGateError> {
        loop {
            let next = state.order.next_emission_sequence().map_err(order_error)?;
            if !state.emitter_active && next == self.start_sequence() {
                state.emitter_active = true;
                return Ok(state);
            }
            state.waiting_emitters += 1;
            self.backend.changed.notify_all();
            state = self.backend.changed.wait(state).map_err(|poisoned| {
                let mut state = poisoned.into_inner();
                state.order.fault(GateFault::SynchronizationPoisoned);
                state.emitter_active = false;
                let error = order_error(state.order.fault_error());
                drop(state);
                self.backend.changed.notify_all();
                error
            })?;
            state.waiting_emitters -= 1;
        }
    }
}

impl Drop for BlockingObservationReservation {
    fn drop(&mut self) {
        if self.armed {
            self.armed = false;
            self.backend.fault(GateFault::ReservationDropped);
        }
    }
}

#[cfg(test)]
mod tests;
